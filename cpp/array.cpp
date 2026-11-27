#include "internal.hpp"

#include <limits>

namespace {

bool check_array(const UsagiObject *array) noexcept {
  if (!usagi::valid_object(array)) {
    return false;
  }
  if (array->kind != USAGI_OBJECT_KIND_ARRAY) {
    usagi::set_error("object is not a java array");
    return false;
  }
  return true;
}

bool check_primitive_array(const UsagiObject *array, int element_kind) noexcept {
  if (!check_array(array)) {
    return false;
  }
  if (!usagi::valid_primitive_array_kind(element_kind)) {
    usagi::set_error("invalid primitive array kind");
    return false;
  }
  if (array->array_kind != element_kind) {
    usagi::set_error("array element type mismatch");
    return false;
  }
  return true;
}

bool check_object_array(const UsagiObject *array) noexcept {
  if (!check_array(array)) {
    return false;
  }
  if (array->array_kind != USAGI_ARRAY_KIND_OBJECT) {
    usagi::set_error("array is not an Object array");
    return false;
  }
  return true;
}

} // namespace

extern "C" {

UsagiObject *usagi_vm_new_primitive_array(UsagiVm *vm, int array_kind,
                                          size_t length) {
  return usagi::catch_to_ptr<UsagiObject>(
      [&]() -> UsagiObject * {
        if (!usagi::valid_vm(vm)) {
          return nullptr;
        }
        if (!usagi::valid_primitive_array_kind(array_kind)) {
          usagi::set_error("invalid primitive array kind");
          return nullptr;
        }
        if (length > static_cast<size_t>(std::numeric_limits<jsize>::max())) {
          usagi::set_error("array length exceeds maximum JNI length");
          return nullptr;
        }

        JNIEnv *env = usagi::current_env(vm);
        if (!env) {
          return nullptr;
        }

        const jsize jlen = static_cast<jsize>(length);
        jarray local = nullptr;
        switch (array_kind) {
        case USAGI_ARRAY_KIND_BOOLEAN:
          local = env->NewBooleanArray(jlen);
          break;
        case USAGI_ARRAY_KIND_BYTE:
          local = env->NewByteArray(jlen);
          break;
        case USAGI_ARRAY_KIND_CHAR:
          local = env->NewCharArray(jlen);
          break;
        case USAGI_ARRAY_KIND_SHORT:
          local = env->NewShortArray(jlen);
          break;
        case USAGI_ARRAY_KIND_INT:
          local = env->NewIntArray(jlen);
          break;
        case USAGI_ARRAY_KIND_LONG:
          local = env->NewLongArray(jlen);
          break;
        case USAGI_ARRAY_KIND_FLOAT:
          local = env->NewFloatArray(jlen);
          break;
        case USAGI_ARRAY_KIND_DOUBLE:
          local = env->NewDoubleArray(jlen);
          break;
        default:
          usagi::set_error("invalid primitive array kind");
          return nullptr;
        }

        if (!local) {
          usagi::take_exception(env);
          usagi::set_error("New<Type>Array returned null");
          return nullptr;
        }

        usagi::LocalRefGuard guard{env, local};
        UsagiObject *object = usagi::make_object_handle(
            vm->inner, env, local, USAGI_OBJECT_KIND_ARRAY);
        if (object) {
          usagi::clear_error();
        }
        return object;
      },
      "unexpected exception creating primitive array");
}

UsagiObject *usagi_vm_new_object_array(UsagiVm *vm, size_t length,
                                       const UsagiObject *element_class,
                                       const UsagiObject *init) {
  return usagi::catch_to_ptr<UsagiObject>(
      [&]() -> UsagiObject * {
        if (!usagi::valid_vm(vm) || !usagi::valid_object(element_class)) {
          return nullptr;
        }
        if (element_class->kind != USAGI_OBJECT_KIND_CLASS) {
          usagi::set_error("element_class is not a java class");
          return nullptr;
        }
        if (element_class->owner != vm->inner) {
          usagi::set_error("element_class belongs to a different vm");
          return nullptr;
        }
        if (init) {
          if (!usagi::valid_object(init)) {
            return nullptr;
          }
          if (init->owner != vm->inner) {
            usagi::set_error("initial element belongs to a different vm");
            return nullptr;
          }
        }
        if (length > static_cast<size_t>(std::numeric_limits<jsize>::max())) {
          usagi::set_error("array length exceeds maximum JNI length");
          return nullptr;
        }

        JNIEnv *env = usagi::current_env(vm);
        if (!env) {
          return nullptr;
        }

        jobject local_init =
            init ? env->NewLocalRef(init->global_ref) : nullptr;
        usagi::LocalRefGuard init_guard{env, local_init};

        jobjectArray local = env->NewObjectArray(
            static_cast<jsize>(length),
            static_cast<jclass>(element_class->global_ref), local_init);
        if (!local) {
          usagi::take_exception(env);
          usagi::set_error("NewObjectArray returned null");
          return nullptr;
        }

        usagi::LocalRefGuard guard{env, local};
        UsagiObject *object = usagi::make_object_handle(
            vm->inner, env, local, USAGI_OBJECT_KIND_ARRAY);
        if (object) {
          usagi::clear_error();
        }
        return object;
      },
      "unexpected exception creating object array");
}

jint usagi_array_length(const UsagiObject *array, size_t *out_length) {
  return usagi::catch_to_status(
      [&]() -> jint {
        if (!out_length) {
          usagi::set_error("null output length pointer");
          return USAGI_ERROR;
        }
        if (!check_array(array)) {
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(array->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        const jsize len =
            env->GetArrayLength(static_cast<jarray>(array->global_ref));
        if (len < 0) {
          usagi::take_exception(env);
          usagi::set_error("failed to get java array length");
          return USAGI_ERROR;
        }

        *out_length = static_cast<size_t>(len);
        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception getting array length");
}

jint usagi_object_array_get(const UsagiObject *array, size_t index,
                            UsagiObject **out_element) {
  if (out_element) {
    *out_element = nullptr;
  }

  return usagi::catch_to_status(
      [&]() -> jint {
        if (!out_element) {
          usagi::set_error("null output element pointer");
          return USAGI_ERROR;
        }
        if (!check_object_array(array)) {
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(array->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        auto arr = static_cast<jobjectArray>(array->global_ref);
        const jsize len = env->GetArrayLength(arr);
        if (len < 0 || index >= static_cast<size_t>(len)) {
          usagi::set_error("array index out of bounds");
          return USAGI_ERROR;
        }

        jobject elem = env->GetObjectArrayElement(arr, static_cast<jsize>(index));
        usagi::LocalRefGuard guard{env, elem};
        if (usagi::take_exception(env)) {
          usagi::set_error("failed to read object array element");
          return USAGI_ERROR;
        }

        if (elem) {
          UsagiObject *handle = usagi::make_object_handle(
              array->owner, env, elem, USAGI_OBJECT_KIND_OBJECT);
          if (!handle) {
            return USAGI_ERROR;
          }
          *out_element = handle;
        }

        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception getting object array element");
}

jint usagi_object_array_set(UsagiObject *array, size_t index,
                            const UsagiObject *value) {
  return usagi::catch_to_status(
      [&]() -> jint {
        if (!check_object_array(array)) {
          return USAGI_ERROR;
        }
        if (value) {
          if (!usagi::valid_object(value)) {
            return USAGI_ERROR;
          }
          if (value->owner != array->owner) {
            usagi::set_error("element belongs to a different vm");
            return USAGI_ERROR;
          }
        }

        JNIEnv *env = usagi::current_env(array->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        auto arr = static_cast<jobjectArray>(array->global_ref);
        const jsize len = env->GetArrayLength(arr);
        if (len < 0 || index >= static_cast<size_t>(len)) {
          usagi::set_error("array index out of bounds");
          return USAGI_ERROR;
        }

        jobject local_val =
            value ? env->NewLocalRef(value->global_ref) : nullptr;
        usagi::LocalRefGuard val_guard{env, local_val};

        env->SetObjectArrayElement(arr, static_cast<jsize>(index), local_val);
        if (usagi::take_exception(env)) {
          usagi::set_error("failed to set object array element");
          return USAGI_ERROR;
        }

        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception setting object array element");
}

jint usagi_array_get_region(const UsagiObject *array, int element_kind,
                            size_t start, size_t length, void *buffer) {
  return usagi::catch_to_status(
      [&]() -> jint {
        if (!check_primitive_array(array, element_kind)) {
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(array->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        auto arr = static_cast<jarray>(array->global_ref);
        const jsize total = env->GetArrayLength(arr);
        if (total < 0) {
          usagi::set_error("failed to get array length");
          return USAGI_ERROR;
        }
        const size_t total_sz = static_cast<size_t>(total);
        if (start > total_sz || length > total_sz - start) {
          usagi::set_error("array region out of bounds");
          return USAGI_ERROR;
        }

        if (length == 0) {
          usagi::clear_error();
          return USAGI_OK;
        }

        if (!buffer) {
          usagi::set_error("null output buffer");
          return USAGI_ERROR;
        }

        const jsize jstart = static_cast<jsize>(start);
        const jsize jlen = static_cast<jsize>(length);

        switch (element_kind) {
        case USAGI_ARRAY_KIND_BOOLEAN: {
          auto *dst = static_cast<jboolean *>(buffer);
          env->GetBooleanArrayRegion(static_cast<jbooleanArray>(arr), jstart,
                                     jlen, dst);
          for (size_t i = 0; i < length; ++i) {
            dst[i] = dst[i] != JNI_FALSE ? JNI_TRUE : JNI_FALSE;
          }
          break;
        }
        case USAGI_ARRAY_KIND_BYTE:
          env->GetByteArrayRegion(static_cast<jbyteArray>(arr), jstart, jlen,
                                  static_cast<jbyte *>(buffer));
          break;
        case USAGI_ARRAY_KIND_CHAR:
          env->GetCharArrayRegion(static_cast<jcharArray>(arr), jstart, jlen,
                                  static_cast<jchar *>(buffer));
          break;
        case USAGI_ARRAY_KIND_SHORT:
          env->GetShortArrayRegion(static_cast<jshortArray>(arr), jstart, jlen,
                                   static_cast<jshort *>(buffer));
          break;
        case USAGI_ARRAY_KIND_INT:
          env->GetIntArrayRegion(static_cast<jintArray>(arr), jstart, jlen,
                                 static_cast<jint *>(buffer));
          break;
        case USAGI_ARRAY_KIND_LONG:
          env->GetLongArrayRegion(static_cast<jlongArray>(arr), jstart, jlen,
                                  static_cast<jlong *>(buffer));
          break;
        case USAGI_ARRAY_KIND_FLOAT:
          env->GetFloatArrayRegion(static_cast<jfloatArray>(arr), jstart, jlen,
                                   static_cast<jfloat *>(buffer));
          break;
        case USAGI_ARRAY_KIND_DOUBLE:
          env->GetDoubleArrayRegion(static_cast<jdoubleArray>(arr), jstart,
                                    jlen, static_cast<jdouble *>(buffer));
          break;
        default:
          usagi::set_error("invalid primitive array kind");
          return USAGI_ERROR;
        }

        if (usagi::take_exception(env)) {
          usagi::set_error("failed to read array region");
          return USAGI_ERROR;
        }

        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception getting array region");
}

jint usagi_array_set_region(UsagiObject *array, int element_kind,
                            size_t start, size_t length, const void *buffer) {
  return usagi::catch_to_status(
      [&]() -> jint {
        if (!check_primitive_array(array, element_kind)) {
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(array->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        auto arr = static_cast<jarray>(array->global_ref);
        const jsize total = env->GetArrayLength(arr);
        if (total < 0) {
          usagi::set_error("failed to get array length");
          return USAGI_ERROR;
        }
        const size_t total_sz = static_cast<size_t>(total);
        if (start > total_sz || length > total_sz - start) {
          usagi::set_error("array region out of bounds");
          return USAGI_ERROR;
        }

        if (length == 0) {
          usagi::clear_error();
          return USAGI_OK;
        }

        if (!buffer) {
          usagi::set_error("null input buffer");
          return USAGI_ERROR;
        }

        const jsize jstart = static_cast<jsize>(start);
        const jsize jlen = static_cast<jsize>(length);

        switch (element_kind) {
        case USAGI_ARRAY_KIND_BOOLEAN:
          env->SetBooleanArrayRegion(static_cast<jbooleanArray>(arr), jstart,
                                     jlen, static_cast<const jboolean *>(buffer));
          break;
        case USAGI_ARRAY_KIND_BYTE:
          env->SetByteArrayRegion(static_cast<jbyteArray>(arr), jstart, jlen,
                                  static_cast<const jbyte *>(buffer));
          break;
        case USAGI_ARRAY_KIND_CHAR:
          env->SetCharArrayRegion(static_cast<jcharArray>(arr), jstart, jlen,
                                  static_cast<const jchar *>(buffer));
          break;
        case USAGI_ARRAY_KIND_SHORT:
          env->SetShortArrayRegion(static_cast<jshortArray>(arr), jstart, jlen,
                                   static_cast<const jshort *>(buffer));
          break;
        case USAGI_ARRAY_KIND_INT:
          env->SetIntArrayRegion(static_cast<jintArray>(arr), jstart, jlen,
                                 static_cast<const jint *>(buffer));
          break;
        case USAGI_ARRAY_KIND_LONG:
          env->SetLongArrayRegion(static_cast<jlongArray>(arr), jstart, jlen,
                                  static_cast<const jlong *>(buffer));
          break;
        case USAGI_ARRAY_KIND_FLOAT:
          env->SetFloatArrayRegion(static_cast<jfloatArray>(arr), jstart, jlen,
                                   static_cast<const jfloat *>(buffer));
          break;
        case USAGI_ARRAY_KIND_DOUBLE:
          env->SetDoubleArrayRegion(static_cast<jdoubleArray>(arr), jstart,
                                    jlen, static_cast<const jdouble *>(buffer));
          break;
        default:
          usagi::set_error("invalid primitive array kind");
          return USAGI_ERROR;
        }

        if (usagi::take_exception(env)) {
          usagi::set_error("failed to write array region");
          return USAGI_ERROR;
        }

        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception setting array region");
}

jint usagi_array_get_elements(UsagiObject *array, int element_kind,
                              void **out_ptr, jboolean *out_is_copy) {
  if (out_ptr) {
    *out_ptr = nullptr;
  }
  if (out_is_copy) {
    *out_is_copy = JNI_FALSE;
  }

  return usagi::catch_to_status(
      [&]() -> jint {
        if (!out_ptr) {
          usagi::set_error("null output pointer");
          return USAGI_ERROR;
        }
        if (!check_primitive_array(array, element_kind)) {
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(array->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        auto arr = static_cast<jarray>(array->global_ref);
        jboolean is_copy = JNI_FALSE;
        void *ptr = nullptr;

        switch (element_kind) {
        case USAGI_ARRAY_KIND_BOOLEAN:
          ptr = env->GetBooleanArrayElements(static_cast<jbooleanArray>(arr),
                                             &is_copy);
          break;
        case USAGI_ARRAY_KIND_BYTE:
          ptr = env->GetByteArrayElements(static_cast<jbyteArray>(arr),
                                          &is_copy);
          break;
        case USAGI_ARRAY_KIND_CHAR:
          ptr = env->GetCharArrayElements(static_cast<jcharArray>(arr),
                                          &is_copy);
          break;
        case USAGI_ARRAY_KIND_SHORT:
          ptr = env->GetShortArrayElements(static_cast<jshortArray>(arr),
                                           &is_copy);
          break;
        case USAGI_ARRAY_KIND_INT:
          ptr =
              env->GetIntArrayElements(static_cast<jintArray>(arr), &is_copy);
          break;
        case USAGI_ARRAY_KIND_LONG:
          ptr = env->GetLongArrayElements(static_cast<jlongArray>(arr),
                                          &is_copy);
          break;
        case USAGI_ARRAY_KIND_FLOAT:
          ptr = env->GetFloatArrayElements(static_cast<jfloatArray>(arr),
                                           &is_copy);
          break;
        case USAGI_ARRAY_KIND_DOUBLE:
          ptr = env->GetDoubleArrayElements(static_cast<jdoubleArray>(arr),
                                            &is_copy);
          break;
        default:
          usagi::set_error("invalid primitive array kind");
          return USAGI_ERROR;
        }

        if (usagi::take_exception(env)) {
          usagi::set_error("failed to pin array elements");
          return USAGI_ERROR;
        }

        const jsize len = env->GetArrayLength(arr);
        if (!ptr && len > 0) {
          usagi::set_error("Get<Type>ArrayElements returned null");
          return USAGI_ERROR;
        }

        *out_ptr = ptr;
        if (out_is_copy) {
          *out_is_copy = is_copy != JNI_FALSE ? JNI_TRUE : JNI_FALSE;
        }
        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception pinning array elements");
}

jint usagi_array_release_elements(UsagiObject *array, int element_kind,
                                  void *ptr, jint mode) {
  return usagi::catch_to_status(
      [&]() -> jint {
        if (!check_primitive_array(array, element_kind)) {
          return USAGI_ERROR;
        }
        if (!ptr) {
          usagi::clear_error();
          return USAGI_OK;
        }

        JNIEnv *env = usagi::current_env(array->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        auto arr = static_cast<jarray>(array->global_ref);
        switch (element_kind) {
        case USAGI_ARRAY_KIND_BOOLEAN:
          env->ReleaseBooleanArrayElements(static_cast<jbooleanArray>(arr),
                                           static_cast<jboolean *>(ptr), mode);
          break;
        case USAGI_ARRAY_KIND_BYTE:
          env->ReleaseByteArrayElements(static_cast<jbyteArray>(arr),
                                        static_cast<jbyte *>(ptr), mode);
          break;
        case USAGI_ARRAY_KIND_CHAR:
          env->ReleaseCharArrayElements(static_cast<jcharArray>(arr),
                                        static_cast<jchar *>(ptr), mode);
          break;
        case USAGI_ARRAY_KIND_SHORT:
          env->ReleaseShortArrayElements(static_cast<jshortArray>(arr),
                                         static_cast<jshort *>(ptr), mode);
          break;
        case USAGI_ARRAY_KIND_INT:
          env->ReleaseIntArrayElements(static_cast<jintArray>(arr),
                                       static_cast<jint *>(ptr), mode);
          break;
        case USAGI_ARRAY_KIND_LONG:
          env->ReleaseLongArrayElements(static_cast<jlongArray>(arr),
                                        static_cast<jlong *>(ptr), mode);
          break;
        case USAGI_ARRAY_KIND_FLOAT:
          env->ReleaseFloatArrayElements(static_cast<jfloatArray>(arr),
                                         static_cast<jfloat *>(ptr), mode);
          break;
        case USAGI_ARRAY_KIND_DOUBLE:
          env->ReleaseDoubleArrayElements(static_cast<jdoubleArray>(arr),
                                          static_cast<jdouble *>(ptr), mode);
          break;
        default:
          usagi::set_error("invalid primitive array kind");
          return USAGI_ERROR;
        }

        if (usagi::take_exception(env)) {
          usagi::set_error("failed to release array elements");
          return USAGI_ERROR;
        }

        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception releasing array elements");
}

} // extern "C"
