use shared_files::core_header::{self};

#[unsafe(no_mangle)]
extern "C" fn module_startup(_core: &core_header::CoreH, _args: &mut Vec<String>) {
    println!("Hello world!");
}

#[unsafe(no_mangle)]
extern "C" fn module_shutdown(_core: &core_header::CoreH) {
    println!("Goodbye world!");
}
