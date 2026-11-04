use crate::data;

#[data(crate = crate, copy)]
pub enum Signal {
    /// SIGHUP.
    HangUp,
}
