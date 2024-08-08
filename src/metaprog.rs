use std::any::TypeId;

/// Test if the type `T` is the same as `U` (need no value)
pub fn type_eq<T: ?Sized + 'static, U: ?Sized + 'static>() -> bool {
    TypeId::of::<T>() == TypeId::of::<U>()
}

#[test]
fn test_type_eq() {
    assert!(type_eq::<String, String>());
    assert!(!type_eq::<&str, String>());
    assert!(!type_eq::<String, i32>());
}
