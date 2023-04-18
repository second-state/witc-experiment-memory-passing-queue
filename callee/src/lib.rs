#![feature(wasm_abi)]

use serde::{Deserialize, Serialize};

#[link(wasm_import_module = "wasmedge.component.model")]
extern "wasm" {
    fn write(offset: usize, len: usize);
    fn read() -> (u32, u32);
}

#[derive(Serialize, Deserialize, Debug)]
struct Person {
    name: String,
    age: u32,
}

fn foo(person: Person, new_age: u32) -> Person {
    Person {
        name: person.name,
        age: new_age,
    }
}

#[no_mangle]
pub unsafe extern "C" fn component_foo() {
    let mut args: Vec<String> = vec![];
    for _ in 0..2 {
        let (offset, len) = read();
        args.push(String::from_raw_parts(
            offset as *mut u8,
            len as usize,
            len as usize,
        ));
    }

    let person: Person = serde_json::from_str(args[0].as_str()).expect("person decode failed");
    let new_age: u32 = serde_json::from_str(args[1].as_str()).expect("age decode failed");

    let new_person = foo(person, new_age);

    let new_person_str = serde_json::to_string(&new_person).unwrap();
    write(new_person_str.as_ptr() as usize, new_person_str.len());
}
