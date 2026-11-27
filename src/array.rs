use std::{
    ffi::{c_int, c_void},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::{
    error::Error,
    ffi,
    object::{GlobalRef, Object, ObjectKind},
    sig::Type,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayKind {
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
    Object,
}

impl ArrayKind {
    pub(crate) fn from_ffi(raw: c_int) -> Option<Self> {
        match raw {
            ffi::USAGI_ARRAY_KIND_BOOLEAN => Some(Self::Boolean),
            ffi::USAGI_ARRAY_KIND_BYTE => Some(Self::Byte),
            ffi::USAGI_ARRAY_KIND_CHAR => Some(Self::Char),
            ffi::USAGI_ARRAY_KIND_SHORT => Some(Self::Short),
            ffi::USAGI_ARRAY_KIND_INT => Some(Self::Int),
            ffi::USAGI_ARRAY_KIND_LONG => Some(Self::Long),
            ffi::USAGI_ARRAY_KIND_FLOAT => Some(Self::Float),
            ffi::USAGI_ARRAY_KIND_DOUBLE => Some(Self::Double),
            ffi::USAGI_ARRAY_KIND_OBJECT => Some(Self::Object),
            _ => None,
        }
    }

    pub(crate) fn to_ffi(self) -> c_int {
        match self {
            Self::Boolean => ffi::USAGI_ARRAY_KIND_BOOLEAN,
            Self::Byte => ffi::USAGI_ARRAY_KIND_BYTE,
            Self::Char => ffi::USAGI_ARRAY_KIND_CHAR,
            Self::Short => ffi::USAGI_ARRAY_KIND_SHORT,
            Self::Int => ffi::USAGI_ARRAY_KIND_INT,
            Self::Long => ffi::USAGI_ARRAY_KIND_LONG,
            Self::Float => ffi::USAGI_ARRAY_KIND_FLOAT,
            Self::Double => ffi::USAGI_ARRAY_KIND_DOUBLE,
            Self::Object => ffi::USAGI_ARRAY_KIND_OBJECT,
        }
    }

    pub(crate) fn matches_type(self, element: &Type) -> bool {
        matches!(
            (self, element),
            (Self::Boolean, Type::Boolean)
                | (Self::Byte, Type::Byte)
                | (Self::Char, Type::Char)
                | (Self::Short, Type::Short)
                | (Self::Int, Type::Int)
                | (Self::Long, Type::Long)
                | (Self::Float, Type::Float)
                | (Self::Double, Type::Double)
                | (Self::Object, Type::Object(_) | Type::Array(_))
        )
    }
}

mod sealed {
    pub trait Sealed {}
}

/// JNI primitive array element types (bool, i8, u16, i16, i32, i64, f32, f64).
pub trait ArrayElement: sealed::Sealed + Copy + Default {
    const KIND: ArrayKind;

    #[doc(hidden)]
    fn read_region(array: &JavaArray<'_>, start: usize, buf: &mut [Self]) -> Result<(), Error>;

    #[doc(hidden)]
    fn write_region(array: &JavaArray<'_>, start: usize, buf: &[Self]) -> Result<(), Error>;
}

/// Primitive types safe to expose as direct pinned slices (i8, u16, i16, i32, i64, f32, f64).
/// bool is excluded because JNI jboolean bytes from native code may not be 0 or 1.
pub trait PinElement: ArrayElement {}

macro_rules! impl_array_element {
    ($ty:ty, $kind:expr) => {
        impl sealed::Sealed for $ty {}
        impl ArrayElement for $ty {
            const KIND: ArrayKind = $kind;

            fn read_region(
                array: &JavaArray<'_>,
                start: usize,
                buf: &mut [Self],
            ) -> Result<(), Error> {
                let ptr = if buf.is_empty() {
                    std::ptr::null_mut()
                } else {
                    buf.as_mut_ptr().cast::<c_void>()
                };
                let status = unsafe {
                    ffi::usagi_array_get_region(
                        array.as_object().raw().as_ptr(),
                        Self::KIND.to_ffi(),
                        start,
                        buf.len(),
                        ptr,
                    )
                };
                if status != ffi::USAGI_OK {
                    return Err(Error::from_native(
                        "array region read",
                        "failed to read array region",
                    ));
                }
                Ok(())
            }

            fn write_region(
                array: &JavaArray<'_>,
                start: usize,
                buf: &[Self],
            ) -> Result<(), Error> {
                let ptr = if buf.is_empty() {
                    std::ptr::null()
                } else {
                    buf.as_ptr().cast::<c_void>()
                };
                let status = unsafe {
                    ffi::usagi_array_set_region(
                        array.as_object().raw().as_ptr(),
                        Self::KIND.to_ffi(),
                        start,
                        buf.len(),
                        ptr,
                    )
                };
                if status != ffi::USAGI_OK {
                    return Err(Error::from_native(
                        "array region write",
                        "failed to write array region",
                    ));
                }
                Ok(())
            }
        }
        impl PinElement for $ty {}
    };
}

impl sealed::Sealed for bool {}
impl ArrayElement for bool {
    const KIND: ArrayKind = ArrayKind::Boolean;

    fn read_region(array: &JavaArray<'_>, start: usize, buf: &mut [Self]) -> Result<(), Error> {
        let mut raw = vec![0u8; buf.len()];
        let ptr = if raw.is_empty() {
            std::ptr::null_mut()
        } else {
            raw.as_mut_ptr().cast::<c_void>()
        };
        let status = unsafe {
            ffi::usagi_array_get_region(
                array.as_object().raw().as_ptr(),
                Self::KIND.to_ffi(),
                start,
                buf.len(),
                ptr,
            )
        };
        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "array region read",
                "failed to read boolean array region",
            ));
        }
        for (dst, src) in buf.iter_mut().zip(raw) {
            *dst = src != 0;
        }
        Ok(())
    }

    fn write_region(array: &JavaArray<'_>, start: usize, buf: &[Self]) -> Result<(), Error> {
        let raw: Vec<u8> = buf.iter().map(|&b| u8::from(b)).collect();
        let ptr = if raw.is_empty() {
            std::ptr::null()
        } else {
            raw.as_ptr().cast::<c_void>()
        };
        let status = unsafe {
            ffi::usagi_array_set_region(
                array.as_object().raw().as_ptr(),
                Self::KIND.to_ffi(),
                start,
                buf.len(),
                ptr,
            )
        };
        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "array region write",
                "failed to write boolean array region",
            ));
        }
        Ok(())
    }
}

impl_array_element!(i8, ArrayKind::Byte);
impl_array_element!(u16, ArrayKind::Char);
impl_array_element!(i16, ArrayKind::Short);
impl_array_element!(i32, ArrayKind::Int);
impl_array_element!(i64, ArrayKind::Long);
impl_array_element!(f32, ArrayKind::Float);
impl_array_element!(f64, ArrayKind::Double);

#[derive(Debug, Clone)]
pub struct JavaArray<'env> {
    object: Object<'env>,
}

impl<'env> JavaArray<'env> {
    pub(crate) fn from_object(object: Object<'env>) -> Self {
        Self { object }
    }

    pub fn as_object(&self) -> &Object<'env> {
        &self.object
    }

    pub fn to_global(&self) -> GlobalRef {
        self.object.to_global()
    }

    pub fn into_global(self) -> GlobalRef {
        self.object.into_global()
    }

    pub fn kind(&self) -> ArrayKind {
        match self.object.kind() {
            ObjectKind::Array(kind) => kind,
            _ => unreachable!("JavaArray always wraps an array UsagiObject"),
        }
    }

    pub fn len(&self) -> Result<usize, Error> {
        let mut length = 0usize;
        let status = unsafe { ffi::usagi_array_length(self.object.raw().as_ptr(), &mut length) };
        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "array length",
                "failed to get java array length",
            ));
        }
        Ok(length)
    }

    pub fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == 0)
    }

    pub fn get_object_element(&self, index: usize) -> Result<Option<Object<'env>>, Error> {
        let mut out_element: *mut ffi::UsagiObject = std::ptr::null_mut();
        let status = unsafe {
            ffi::usagi_object_array_get(self.object.raw().as_ptr(), index, &mut out_element)
        };
        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "object array get",
                "failed to get object array element",
            ));
        }
        if out_element.is_null() {
            return Ok(None);
        }
        Object::from_raw(
            std::sync::Arc::clone(self.object.vm_inner()),
            out_element,
            "object array get",
            "invalid object array element",
        )
        .map(Some)
    }

    pub fn set_object_element(
        &self,
        index: usize,
        value: Option<&Object<'_>>,
    ) -> Result<(), Error> {
        if let Some(obj) = value {
            if !obj.belongs_to_inner(self.object.vm_inner()) {
                return Err(Error::new(
                    "object array set",
                    "element belongs to a different vm",
                ));
            }
        }
        let value_ptr = value.map_or(std::ptr::null(), |o| o.raw().as_ptr().cast_const());
        let status =
            unsafe { ffi::usagi_object_array_set(self.object.raw().as_ptr(), index, value_ptr) };
        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "object array set",
                "failed to set object array element",
            ));
        }
        Ok(())
    }

    pub fn get_region<T: ArrayElement>(&self, start: usize, buf: &mut [T]) -> Result<(), Error> {
        T::read_region(self, start, buf)
    }

    pub fn set_region<T: ArrayElement>(&self, start: usize, buf: &[T]) -> Result<(), Error> {
        T::write_region(self, start, buf)
    }

    pub fn to_vec<T: ArrayElement>(&self) -> Result<Vec<T>, Error> {
        let len = self.len()?;
        let mut vec = vec![T::default(); len];
        self.get_region(0, &mut vec)?;
        Ok(vec)
    }

    /// Pins the primitive array's elements via Get<Type>ArrayElements and releases on drop.
    ///
    /// # Safety
    /// The caller must ensure no other code (Rust or native) mutates or creates an aliasing
    /// mutable slice over the same underlying array elements while the guard is alive.
    pub unsafe fn pin<T: PinElement>(&self) -> Result<ArrayElements<'_, T>, Error> {
        let len = self.len()?;
        let mut raw_ptr: *mut c_void = std::ptr::null_mut();
        let mut is_copy = false;
        let status = unsafe {
            ffi::usagi_array_get_elements(
                self.object.raw().as_ptr(),
                T::KIND.to_ffi(),
                &mut raw_ptr,
                &mut is_copy,
            )
        };
        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "array pin",
                "failed to pin array elements",
            ));
        }

        let slice_ptr = if len == 0 || raw_ptr.is_null() {
            NonNull::dangling()
        } else {
            NonNull::new(raw_ptr.cast::<T>()).expect("checked non-null")
        };

        Ok(ArrayElements {
            raw: self.object.raw(),
            raw_ptr,
            slice_ptr,
            len,
            is_copy,
            released: false,
            _marker: PhantomData,
        })
    }
}

pub struct ArrayElements<'arr, T: PinElement> {
    raw: NonNull<ffi::UsagiObject>,
    raw_ptr: *mut c_void,
    slice_ptr: NonNull<T>,
    len: usize,
    is_copy: bool,
    released: bool,
    _marker: PhantomData<&'arr mut [T]>,
}

impl<T: PinElement> ArrayElements<'_, T> {
    pub fn is_copy(&self) -> bool {
        self.is_copy
    }

    pub fn commit(&mut self) -> Result<(), Error> {
        let status = unsafe {
            ffi::usagi_array_release_elements(
                self.raw.as_ptr(),
                T::KIND.to_ffi(),
                self.raw_ptr,
                ffi::JNI_COMMIT,
            )
        };
        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "array commit",
                "failed to commit array elements",
            ));
        }
        Ok(())
    }

    pub fn abort(mut self) -> Result<(), Error> {
        self.released = true;
        let status = unsafe {
            ffi::usagi_array_release_elements(
                self.raw.as_ptr(),
                T::KIND.to_ffi(),
                self.raw_ptr,
                ffi::JNI_ABORT,
            )
        };
        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "array abort",
                "failed to abort pinned array elements",
            ));
        }
        Ok(())
    }
}

impl<T: PinElement> Deref for ArrayElements<'_, T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.slice_ptr.as_ptr(), self.len) }
    }
}

impl<T: PinElement> DerefMut for ArrayElements<'_, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.slice_ptr.as_ptr(), self.len) }
    }
}

impl<T: PinElement> Drop for ArrayElements<'_, T> {
    fn drop(&mut self) {
        if !self.released {
            let _ = unsafe {
                ffi::usagi_array_release_elements(
                    self.raw.as_ptr(),
                    T::KIND.to_ffi(),
                    self.raw_ptr,
                    0,
                )
            };
        }
    }
}
