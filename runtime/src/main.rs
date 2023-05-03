use anyhow::Error;
use once_cell::sync::Lazy;
use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::{AtomicI32, Ordering},
};
use wasmedge_sdk::{
    config::{CommonConfigOptions, ConfigBuilder, HostRegistrationConfigOptions},
    error::HostFuncError,
    host_function, Caller, ImportObjectBuilder, Vm, WasmValue,
};

struct GrowCache {
    offset: u32,
    pages: u32,
}

struct GlobalState {
    counter: AtomicI32,
    grow_cache: HashMap<String, GrowCache>,
    queue_pool: HashMap<i32, VecDeque<String>>,
}

impl GlobalState {
    fn new() -> Self {
        Self {
            counter: AtomicI32::new(0),
            queue_pool: HashMap::new(),
            grow_cache: HashMap::new(),
        }
    }

    // This allocation algorithm relys on HashMap will limit the bucket size to a fixed number,
    // and the calls will not grow too fast (run out of i32 to use).
    // It still might have problem, if two limits above are broke.
    fn new_queue(&mut self) -> i32 {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        self.queue_pool.insert(id, VecDeque::new());
        id
    }

    fn put_buffer(&mut self, queue_id: i32, buf: String) {
        self.queue_pool.get_mut(&queue_id).unwrap().push_back(buf);
    }

    fn read_buffer(&mut self, queue_id: i32) -> String {
        self.queue_pool
            .get_mut(&queue_id)
            .unwrap()
            .pop_front()
            .unwrap()
    }

    fn get_cache(&self, instance_name: &String) -> Option<&GrowCache> {
        self.grow_cache.get(instance_name)
    }

    fn update_cache(&mut self, instance_name: String, offset: u32, pages: u32) {
        self.grow_cache
            .insert(instance_name, GrowCache { offset, pages });
    }
}

static mut STATE: Lazy<GlobalState> = Lazy::new(|| GlobalState::new());

#[host_function]
fn require_queue(_caller: Caller, _input: Vec<WasmValue>) -> Result<Vec<WasmValue>, HostFuncError> {
    unsafe {
        let id = STATE.new_queue();
        Ok(vec![WasmValue::from_i32(id)])
    }
}

#[host_function]
fn put_buffer(caller: Caller, input: Vec<WasmValue>) -> Result<Vec<WasmValue>, HostFuncError> {
    let id = input[0].to_i32();
    let offset = input[1].to_i32() as u32;
    let len = input[2].to_i32() as u32;

    let data_buffer = caller.memory(0).unwrap().read_string(offset, len).unwrap();

    println!("enqueue `{}`", data_buffer.clone());

    unsafe {
        STATE.put_buffer(id, data_buffer);
    }

    Ok(vec![])
}

#[host_function]
fn read_buffer(caller: Caller, input: Vec<WasmValue>) -> Result<Vec<WasmValue>, HostFuncError> {
    let read_buf_struct_ptr = input[0].to_i32() as u32;
    let queue_id = input[1].to_i32();

    let data_buffer = unsafe { &STATE.read_buffer(queue_id) };
    // capacity will use underlying vector's capacity
    // potential problem is it might be bigger than exact (data) needs
    let data_size = data_buffer.capacity() as u32;
    // A page is 64KiB = 65,536 bytes, and the capacity of a string base on how many u8 it had,
    // exactly how many bytes it had
    let pages = if data_size < 65536 {
        1
    } else {
        data_size / 65536 + 1
    };

    let mut mem = caller.memory(0).unwrap();

    let instance_name = caller.instance().unwrap().name().unwrap();
    let offset = match unsafe { STATE.get_cache(&instance_name) } {
        // 1. cache missing than grow 50
        None => {
            let current_tail = mem.size();
            let offset = current_tail + 1;

            mem.grow(pages).unwrap();
            // 1. memory the `current_tail+1` as `offset`
            // 2. memory the `pages` we just grow
            unsafe {
                STATE.update_cache(instance_name, offset, pages);
            }

            offset
        }
        // 2. cache existed, than reuse `offset` in cache
        Some(cache) => {
            let offset = cache.offset;
            // the size we already have
            let grew_pages = cache.pages;

            // if `grew_pages` isn't big enough then grow more to reach the needed
            if grew_pages < pages {
                mem.grow(pages - grew_pages).unwrap();
                unsafe {
                    // and update the cache
                    STATE.update_cache(instance_name, offset, pages);
                }
            }

            offset
        }
    };

    mem.write(data_buffer, offset).unwrap();

    let instance_ptr = offset as u32;
    let mut struct_content = instance_ptr.to_le_bytes().to_vec();
    // This assuming that the struct `ReadBuf` in instance will have linear layout
    //
    // #[repr(C)]
    // pub struct ReadBuf {
    //     pub offset: usize,
    //     pub len: usize,
    // }
    struct_content.extend((data_buffer.len() as u32).to_le_bytes());
    mem.write(struct_content, read_buf_struct_ptr).unwrap();

    Ok(vec![])
}

fn main() -> Result<(), Error> {
    let config = ConfigBuilder::new(CommonConfigOptions::default())
        .with_host_registration_config(HostRegistrationConfigOptions::default().wasi(true))
        .build()?;

    let import_object = ImportObjectBuilder::new()
        .with_func::<(), i32>("require_queue", require_queue)?
        .with_func::<(i32, i32, i32), ()>("write", put_buffer)?
        .with_func::<(i32, i32), ()>("read", read_buffer)?
        .build("wasmedge.component.model")?;

    let vm = Vm::new(Some(config))?
        .register_import_module(import_object)?
        .register_module_from_file("callee", "target/wasm32-wasi/release/callee.wasm")?
        .register_module_from_file("caller", "target/wasm32-wasi/release/caller.wasm")?;

    let result = vm.run_func(Some("caller"), "start", None)?;
    assert!(result[0].to_i32() == 20);

    Ok(())
}
