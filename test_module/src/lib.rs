use shared_files::core_header::{self};

#[unsafe(no_mangle)]
extern "system" fn module_startup(_core: &core_header::CoreH) {
    println!("Hello world!");
}

#[unsafe(no_mangle)]
extern "system" fn module_shutdown(_core: &core_header::CoreH) {
    println!("Goodbye world!");
}
