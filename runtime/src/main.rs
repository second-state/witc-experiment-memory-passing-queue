use std::collections::VecDeque;

use anyhow::Error;
use wasmedge_sdk::{
    config::{CommonConfigOptions, ConfigBuilder, HostRegistrationConfigOptions},
    error::HostFuncError,
    host_function, Caller, ImportObjectBuilder, Vm, WasmValue,
};

static mut EXCHANGING: VecDeque<String> = VecDeque::new();

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

    println!("dequeue {}", data_buffer);

    let mut mem = caller.memory(0).unwrap();

    let current_tail = mem.size();
    // TODO:
    // Grow 100 is "enough" in this example, but to have "correct" behavior, we must use grow by
    // need and have a cache of previous grew offset & grew size.
    //
    // For example, if we have string need size 50, it should first look at cache
    // 1. cache missing than grow 50
    //    1. memory the `current_tail+1` as `offset`
    //    2. memory the `grow_size` as `50`
    // 2. cache existed, than reuse `offset` in cache
    //    1. if `grow_size` is big enough than reuse it
    //    2. or grow more to reach the needed, than update the cache
    mem.grow(100).unwrap();
    let offset = current_tail + 1;
    mem.write(data_buffer, offset).unwrap();

    Ok(vec![
        WasmValue::from_i32(offset as i32),
        WasmValue::from_i32(data_buffer.len() as i32),
    ])
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
