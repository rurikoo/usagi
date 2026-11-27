#include "internal.hpp"

extern "C" {

UsagiObject *usagi_vm_new_direct_byte_buffer(UsagiVm *vm, void *address,
                                             jlong capacity) {
  return usagi::catch_to_ptr<UsagiObject>(
      [&]() -> UsagiObject * {
        if (!usagi::valid_vm(vm)) {
          return nullptr;
        }
        if (!address) {
          usagi::set_error("null direct buffer address");
          return nullptr;
        }
        if (capacity < 0) {
          usagi::set_error("negative direct buffer capacity");
          return nullptr;
        }

        JNIEnv *env = usagi::current_env(vm);
        if (!env) {
          return nullptr;
        }

        jobject local = env->NewDirectByteBuffer(address, capacity);
        if (!local) {
          usagi::take_exception(env);
          usagi::set_error("NewDirectByteBuffer returned null");
          return nullptr;
        }

        usagi::LocalRefGuard guard{env, local};
        UsagiObject *object = usagi::make_object_handle(
            vm->inner, env, local, USAGI_OBJECT_KIND_OBJECT);
        if (object) {
          usagi::clear_error();
        }
        return object;
      },
      "unexpected exception creating direct ByteBuffer");
}

jint usagi_direct_buffer_address(const UsagiObject *buffer, void **out_address) {
  if (out_address) {
    *out_address = nullptr;
  }

  return usagi::catch_to_status(
      [&]() -> jint {
        if (!out_address) {
          usagi::set_error("null output address pointer");
          return USAGI_ERROR;
        }
        if (!usagi::valid_object(buffer)) {
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(buffer->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        void *addr = env->GetDirectBufferAddress(buffer->global_ref);
        if (usagi::take_exception(env) || !addr) {
          if (usagi::current_error().empty()) {
            usagi::set_error("object is not a direct ByteBuffer");
          }
          return USAGI_ERROR;
        }

        *out_address = addr;
        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception getting direct buffer address");
}

jint usagi_direct_buffer_capacity(const UsagiObject *buffer,
                                  jlong *out_capacity) {
  if (out_capacity) {
    *out_capacity = -1;
  }

  return usagi::catch_to_status(
      [&]() -> jint {
        if (!out_capacity) {
          usagi::set_error("null output capacity pointer");
          return USAGI_ERROR;
        }
        if (!usagi::valid_object(buffer)) {
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(buffer->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        const jlong cap = env->GetDirectBufferCapacity(buffer->global_ref);
        if (usagi::take_exception(env) || cap < 0) {
          if (usagi::current_error().empty()) {
            usagi::set_error("object is not a direct ByteBuffer");
          }
          return USAGI_ERROR;
        }

        *out_capacity = cap;
        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception getting direct buffer capacity");
}

} // extern "C"
