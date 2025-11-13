pub const FILE_EXTENSION: &'static str = ".ppcb";

pub struct CoreH {
    pub ping_core_f: fn(),
}

pub fn ping_core(core: &CoreH) {
    (core.ping_core_f)()
}
