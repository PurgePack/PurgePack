use shared_files::core_header;

#[unsafe(no_mangle)]
extern "system" fn module_startup(_core: &core_header::CoreH) {
    println!("Hello from RLS module!");
}

#[unsafe(no_mangle)]
extern "system" fn module_shutdown(_core: &mut core_header::CoreH, _exiting: bool) {
    println!("Goodbye from RLS module!");
}
