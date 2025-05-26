pub trait HasErrorValue {
    fn is_error_value(&self) -> bool;
    fn into_error_value(self) -> Self;
}

impl HasErrorValue for i32 {
    fn is_error_value(&self) -> bool { 
        *self == -1
    }
    fn into_error_value(self) -> Self {
        -1
    }
}