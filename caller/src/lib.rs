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
extern "C" {
    fn require_queue() -> i32;
    fn write(id: i32, offset: usize, len: usize);
    fn read(id: i32) -> ReadBuf;
}

#[derive(Serialize, Deserialize, Debug)]
struct Person {
    name: String,
    age: u32,
}

#[link(wasm_import_module = "callee")]
extern "C" {
    fn component_foo(id: i32);
}

unsafe fn foo(person: Person, new_age: u32) -> Person {
    let id = require_queue();

    let arg1 = serde_json::to_string(&person).unwrap();
    write(id, arg1.as_ptr() as usize, arg1.len());
    let arg2 = serde_json::to_string(&new_age).unwrap();
    write(id, arg2.as_ptr() as usize, arg2.len());

    component_foo(id);
    serde_json::from_str(read(id).to_string().as_str()).unwrap()
}

#[no_mangle]
pub unsafe extern "C" fn start() -> u32 {
    let person = Person {
        name: "John".to_string(),
        age: 18,
    };
    let new_age: u32 = 20;

    assert!(person.age == 18);
    let p = foo(person, new_age);
    assert!(p.age == 20);

    return p.age;
}
