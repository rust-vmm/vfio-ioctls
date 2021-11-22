use std::mem::size_of;

/// Returns a `Vec<T>` with a size in bytes at least as large as `size_in_bytes`.
fn vec_with_size_in_bytes<T: Default>(size_in_bytes: usize) -> Vec<T> {
    let rounded_size = (size_in_bytes + size_of::<T>() - 1) / size_of::<T>();
    let mut v = Vec::with_capacity(rounded_size);
    for _ in 0..rounded_size {
        v.push(T::default())
    }
    v
}

/// The VFIO API has several structs that resemble the following `Foo` structure:
///
/// ```
/// struct ControlMessageHeader {
///     r#type: u8,
///     length: u8,
/// }
///
/// #[repr(C)]
/// pub struct __IncompleteArrayField<T>(::std::marker::PhantomData<T>);
/// #[repr(C)]
/// struct Foo {
///     some_data: ControlMessageHeader,
///     entries: __IncompleteArrayField<u32>,
/// }
/// ```
///
/// In order to allocate such a structure, `size_of::<Foo>()` would be too small because it would not
/// include any space for `entries`. To make the allocation large enough while still being aligned
/// for `Foo`, a `Vec<Foo>` is created. Only the first element of `Vec<Foo>` would actually be used
/// as a `Foo`. The remaining memory in the `Vec<Foo>` is for `entries`, which must be contiguous
/// with `Foo`. This function is used to make the `Vec<Foo>` with enough space for `count` entries.
pub(crate) fn vec_with_array_field<T: Default, F>(count: usize) -> Vec<T> {
    let element_space = count * size_of::<F>();
    let vec_size_bytes = size_of::<T>() + element_space;
    vec_with_size_in_bytes(vec_size_bytes)
}
