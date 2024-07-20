use std::any::TypeId;

pub fn type_eq<T: ?Sized + 'static, U: ?Sized + 'static>() -> bool {
    TypeId::of::<T>() == TypeId::of::<U>()
}
