#include "internal.hpp"

#include <limits>

namespace usagi {

UsagiObject *make_object_handle(const std::shared_ptr<jnivm::VM> &vm,
                                JNIEnv *env, jobject local_ref,
                                UsagiObjectKind kind) {
  if (!vm || !env || !local_ref) {
    set_error("invalid VM, JNI environment, or object reference");
    return nullptr;
  }

  jobject global_ref = env->NewGlobalRef(local_ref);
  if (!global_ref) {
    take_exception(env);
    set_error("failed to create global JNI reference");
    return nullptr;
  }

  auto handle = std::make_unique<UsagiObject>();
  handle->owner = vm;
  handle->global_ref = global_ref;
  handle->kind = kind;
  return handle.release();
}

} // namespace usagi

extern "C" {

UsagiObject *usagi_vm_new_string_utf16(UsagiVm *vm, const uint16_t *units,
                                       size_t length) {
  return usagi::catch_to_ptr<UsagiObject>(
      [&]() -> UsagiObject * {
        if (!usagi::valid_vm(vm)) {
          return nullptr;
        }

        if (length > static_cast<size_t>(std::numeric_limits<jsize>::max())) {
          usagi::set_error("string exceeds maximum JNI length");
          return nullptr;
        }

        if (length != 0 && !units) {
          usagi::set_error("null UTF-16 buffer with nonzero length");
          return nullptr;
        }

        JNIEnv *env = usagi::current_env(vm);
        if (!env) {
          return nullptr;
        }

        static_assert(sizeof(jchar) == sizeof(uint16_t));

        const jchar empty = 0;
        const jchar *chars =
            length == 0 ? &empty : reinterpret_cast<const jchar *>(units);

        jstring local = env->NewString(chars, static_cast<jsize>(length));
        if (!local) {
          usagi::take_exception(env);
          usagi::set_error("NewString returned null");
          return nullptr;
        }

        usagi::LocalRefGuard guard{env, local};
        UsagiObject *object = usagi::make_object_handle(
            vm->inner, env, local, USAGI_OBJECT_KIND_STRING);

        if (object) {
          usagi::clear_error();
        }
        return object;
      },
      "unexpected exception creating Java string");
}

void usagi_object_destroy(UsagiObject *object) {
  if (!object) {
    return;
  }

  try {
    if (object->owner && object->global_ref) {
      JavaVM *java_vm = object->owner->GetJavaVM();
      if (java_vm) {
        JNIEnv *env = nullptr;
        bool attached_here = false;

        const jint status =
            java_vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6);
        if (status != JNI_OK || !env) {
          if (java_vm->AttachCurrentThread(&env, nullptr) == JNI_OK && env) {
            attached_here = true;
          }
        }

        if (env) {
          env->DeleteGlobalRef(object->global_ref);
          object->global_ref = nullptr;
        }

        if (attached_here) {
          java_vm->DetachCurrentThread();
        }
      }
    }
  } catch (...) {
    // destructors must never throw
  }

  delete object;
}

jint usagi_string_utf16_length(const UsagiObject *object, size_t *out_length) {
  return usagi::catch_to_status(
      [&]() -> jint {
        if (!usagi::valid_object(object) || !out_length) {
          usagi::set_error("invalid object or output pointer");
          return USAGI_ERROR;
        }

        if (object->kind != USAGI_OBJECT_KIND_STRING) {
          usagi::set_error("object is not a Java string");
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(object->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        const jsize length =
            env->GetStringLength(static_cast<jstring>(object->global_ref));
        if (length < 0) {
          usagi::take_exception(env);
          usagi::set_error("failed to obtain Java string length");
          return USAGI_ERROR;
        }

        *out_length = static_cast<size_t>(length);
        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception obtaining string length");
}

jint usagi_string_copy_utf16(const UsagiObject *object, uint16_t *buffer,
                             size_t capacity, size_t *out_length) {
  return usagi::catch_to_status(
      [&]() -> jint {
        if (!usagi::valid_object(object) || !out_length) {
          usagi::set_error("invalid object or output pointer");
          return USAGI_ERROR;
        }

        if (object->kind != USAGI_OBJECT_KIND_STRING) {
          usagi::set_error("object is not a Java string");
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(object->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        jstring string = static_cast<jstring>(object->global_ref);
        const jsize length = env->GetStringLength(string);
        if (length < 0) {
          usagi::take_exception(env);
          usagi::set_error("failed to obtain Java string length");
          return USAGI_ERROR;
        }

        const size_t required = static_cast<size_t>(length);
        *out_length = required;

        if (capacity < required) {
          usagi::set_error("UTF-16 output buffer is too small");
          return USAGI_BUFFER_TOO_SMALL;
        }

        if (required != 0 && !buffer) {
          usagi::set_error("null output buffer");
          return USAGI_ERROR;
        }

        const jchar *chars = env->GetStringChars(string, nullptr);
        usagi::StringCharsGuard guard{env, string, chars};

        if (!chars && required != 0) {
          usagi::take_exception(env);
          usagi::set_error("failed to access Java string characters");
          return USAGI_ERROR;
        }

        for (size_t i = 0; i < required; ++i) {
          buffer[i] = static_cast<uint16_t>(chars[i]);
        }

        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception copying Java string");
}

} // extern "C"
