use shared_files::core_header::*;

#[unsafe(no_mangle)]
extern "C" fn module_startup(_core: &CoreH, _args: &mut Vec<String>) {
    println!("Hello world!");
}

#[unsafe(no_mangle)]
extern "C" fn module_shutdown(_core: &CoreH) {
    println!("Goodbye world!");
}
