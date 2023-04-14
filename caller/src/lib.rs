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

#[link(wasm_import_module = "callee")]
extern "C" {
    fn foo(arity: usize) -> usize;
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

    let returns_arity = foo(2);
    let mut returns: Vec<String> = vec![];
    for _ in 0..returns_arity {
        let (offset, len) = read();
        returns.push(String::from_raw_parts(offset as *mut u8, len, len));
    }

    let p: Person = serde_json::from_str(returns[0].as_str()).unwrap();
    println!("{:?}", p);

    return 0;
}
