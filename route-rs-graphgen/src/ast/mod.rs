#[macro_use]
mod token;

mod r#struct;
pub(crate) use crate::ast::r#struct::*;

mod magic;
pub(crate) use crate::ast::magic::*;

mod values;
pub(crate) use crate::ast::values::*;
