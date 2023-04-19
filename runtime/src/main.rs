use anyhow::Error;
use once_cell::sync::Lazy;
use std::collections::{HashMap, VecDeque};
use wasmedge_sdk::{
    config::{CommonConfigOptions, ConfigBuilder, HostRegistrationConfigOptions},
    error::HostFuncError,
    host_function, Caller, ImportObjectBuilder, Vm, WasmValue,
};

struct GrowCache {
    offset: u32,
    pages: u32,
}

static mut EXCHANGING: VecDeque<String> = VecDeque::new();
static mut GREW_SIZE: Lazy<HashMap<String, GrowCache>> = Lazy::new(|| HashMap::new());

#[host_function]
fn put_buffer(caller: Caller, input: Vec<WasmValue>) -> Result<Vec<WasmValue>, HostFuncError> {
    let offset = input[0].to_i32() as u32;
    let len = input[1].to_i32() as u32;

    let data_buffer = caller.memory(0).unwrap().read_string(offset, len).unwrap();

    println!("enqueue {}", data_buffer.clone());

    unsafe {
        EXCHANGING.push_back(data_buffer);
    }

    Ok(vec![])
}

#[host_function]
fn read_buffer(caller: Caller, _input: Vec<WasmValue>) -> Result<Vec<WasmValue>, HostFuncError> {
    let data_buffer = unsafe { &EXCHANGING.pop_front().unwrap() };
    let data_size = (data_buffer.as_bytes().len() * 8) as u32;
    // one page = 64KiB = 65,536 bytes
    let pages = (data_size / (65536)) + 1;

    println!(
        "dequeue `{}`\n pages: {} (64KiB each page)\n data size: {} bytes",
        data_buffer, pages, data_size,
    );

    let mut mem = caller.memory(0).unwrap();

    let instance_name = caller.instance().unwrap().name().unwrap();
    let cache = unsafe { GREW_SIZE.get(&instance_name) };

    match cache {
        // 1. cache missing than grow 50
        None => {
            let current_tail = mem.size();

            mem.grow(pages).unwrap();
            let offset = current_tail + 1;
            // 1. memory the `current_tail+1` as `offset`
            mem.write(data_buffer, offset).unwrap();
            // 2. memory the `pages` we just grow
            unsafe {
                GREW_SIZE.insert(instance_name, GrowCache { offset, pages });
            }

            Ok(vec![
                WasmValue::from_i32(offset as i32),
                WasmValue::from_i32(data_buffer.len() as i32),
            ])
        }
        // 2. cache existed, than reuse `offset` in cache
        Some(cache) => {
            let offset = cache.offset;
            // the size we already have
            let grew_pages = cache.pages;

            if grew_pages >= pages {
                // 1. if `grow_size` is big enough than reuse it
                mem.write(data_buffer, offset).unwrap();
                Ok(vec![
                    WasmValue::from_i32(offset as i32),
                    WasmValue::from_i32(data_buffer.len() as i32),
                ])
            } else {
                // 2. or grow more to reach the needed, than update the cache
                mem.grow(pages - grew_pages).unwrap();
                mem.write(data_buffer, offset).unwrap();
                unsafe {
                    GREW_SIZE.insert(instance_name, GrowCache { offset, pages });
                }

                Ok(vec![
                    WasmValue::from_i32(offset as i32),
                    WasmValue::from_i32(data_buffer.len() as i32),
                ])
            }
        }
    }
}

fn main() -> Result<(), Error> {
    let config = ConfigBuilder::new(CommonConfigOptions::default())
        .with_host_registration_config(HostRegistrationConfigOptions::default().wasi(true))
        .build()?;

    let import_object = ImportObjectBuilder::new()
        .with_func::<(i32, i32), ()>("write", put_buffer)?
        .with_func::<(), (i32, i32)>("read", read_buffer)?
        .build("wasmedge.component.model")?;

    let vm = Vm::new(Some(config))?
        .register_import_module(import_object)?
        .register_module_from_file("callee", "target/wasm32-wasi/release/callee.wasm")?
        .register_module_from_file("caller", "target/wasm32-wasi/release/caller.wasm")?;

    let result = vm.run_func(Some("caller"), "start", None)?;
    assert!(result[0].to_i32() == 0);

    Ok(())
}
