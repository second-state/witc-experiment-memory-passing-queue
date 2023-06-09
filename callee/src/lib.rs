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
    fn write(id: i32, offset: usize, len: usize);
    fn read(id: i32) -> ReadBuf;
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
pub unsafe extern "C" fn component_foo(id: i32) {
    let person: Person =
        serde_json::from_str(read(id).to_string().as_str()).expect("person decode failed");
    let new_age: u32 =
        serde_json::from_str(read(id).to_string().as_str()).expect("age decode failed");
    let new_person = foo(person, new_age);

    let new_person_str = serde_json::to_string(&new_person).unwrap();
    write(id, new_person_str.as_ptr() as usize, new_person_str.len());
}
