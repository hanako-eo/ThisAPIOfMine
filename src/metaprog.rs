use std::any::TypeId;

/// Test if the type `T` is the same as `U` (need no value)
pub fn type_eq<T: ?Sized + 'static, U: ?Sized + 'static>() -> bool {
    TypeId::of::<T>() == TypeId::of::<U>()
}
