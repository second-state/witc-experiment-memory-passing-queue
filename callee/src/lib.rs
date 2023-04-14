use serde::{Deserialize, Serialize};

#[link(wasm_import_module = "wasmedge.component.model")]
extern "C" {
    fn write(offset: usize, len: usize);
    fn read() -> (usize, usize);
}

#[derive(Serialize, Deserialize, Debug)]
struct Person {
    name: String,
    age: u32,
}

#[no_mangle]
pub unsafe extern "C" fn foo(arity: usize) -> usize {
    let mut args: Vec<String> = vec![];
    for _ in 0..arity {
        let (offset, len) = read();
        args.push(String::from_raw_parts(offset as *mut u8, len, len));
    }

    let person: Person = serde_json::from_str(args[0].as_str()).unwrap();
    let new_age: u32 = serde_json::from_str(args[1].as_str()).unwrap();

    let new_person = Person {
        name: person.name,
        age: new_age,
    };

    let new_person_str = serde_json::to_string(&new_person).unwrap();
    write(new_person_str.as_ptr() as usize, new_person_str.len());

    return 1;
}
