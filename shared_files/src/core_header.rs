use std::{cell::RefCell, ffi::OsString, rc::Rc};

pub struct CoreH {
    pub print_fn: fn(&str),
    pub args: Rc<Vec<OsString>>,
    pub message_received: Rc<RefCell<String>>,
    pub cancel_exit: bool,
}

pub fn print_core(core: &CoreH, s: &str) {
    (core.print_fn)(s);
}
