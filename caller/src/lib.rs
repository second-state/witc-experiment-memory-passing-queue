#![feature(wasm_abi)]

use serde::{Deserialize, Serialize};

#[repr(C)]
pub struct ReadBuf {
    pub offset: usize,
    pub len: usize,
}

impl ToString for ReadBuf {
    fn to_string(self: &Self) -> String {
        unsafe { String::from_raw_parts(self.offset as *mut u8, self.len, self.len) }
    }
}

#[link(wasm_import_module = "wasmedge.component.model")]
extern "wasm" {
    fn write(offset: usize, len: usize);
    fn read() -> ReadBuf;

}

#[derive(Serialize, Deserialize, Debug)]
struct Person {
    name: String,
    age: u32,
}

#[link(wasm_import_module = "callee")]
extern "C" {
    fn component_foo();
}

#[no_mangle]
pub unsafe extern "C" fn start() -> u32 {
    let person = Person {
        name: "John".to_string(),
        age: 18,
    };
    let new_age: u32 = 20;

    let arg1 = serde_json::to_string(&person).unwrap();
    write(arg1.as_ptr() as usize, arg1.len());
    let arg2 = serde_json::to_string(&new_age).unwrap();
    write(arg2.as_ptr() as usize, arg2.len());

    component_foo();
    let mut returns: Vec<String> = vec![];
    for _ in 0..1 {
        // NOTE: we must clone this string, because next `read` will reuse this memory block
        returns.push(read().to_string().clone());
    }

    let p: Person = serde_json::from_str(returns[0].as_str()).unwrap();

    return 0;
}
