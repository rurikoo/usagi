#include "internal.hpp"

namespace usagi {

JNIEnv *current_env(const std::shared_ptr<jnivm::VM> &vm) {
  if (!vm) {
    set_error("invalid native VM");
    return nullptr;
  }

  JavaVM *java_vm = vm->GetJavaVM();
  if (!java_vm) {
    set_error("JavaVM unavailable");
    return nullptr;
  }

  JNIEnv *env = nullptr;
  const jint status =
      java_vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6);

  if (status != JNI_OK || !env) {
    set_error("JNI environment unavailable (thread not attached)");
    return nullptr;
  }

  return env;
}

JNIEnv *current_env(UsagiVm *vm) {
  if (!valid_vm(vm)) {
    return nullptr;
  }
  return current_env(vm->inner);
}

} // namespace usagi

extern "C" {

UsagiVm *usagi_vm_create(void) {
  return usagi::catch_to_ptr<UsagiVm>(
      []() -> UsagiVm * {
        auto handle = std::make_unique<UsagiVm>();
        handle->inner = std::make_shared<jnivm::VM>();
        usagi::clear_error();
        return handle.release();
      },
      "failed to create JNI VM");
}

void usagi_vm_destroy(UsagiVm *vm) { delete vm; }

JavaVM *usagi_vm_get_java_vm(UsagiVm *vm) {
  return usagi::catch_to_ptr<JavaVM>(
      [&]() -> JavaVM * {
        if (!usagi::valid_vm(vm)) {
          return nullptr;
        }

        JavaVM *java_vm = vm->inner->GetJavaVM();
        if (!java_vm) {
          usagi::set_error("JavaVM unavailable");
          return nullptr;
        }

        usagi::clear_error();
        return java_vm;
      },
      "failed to get JavaVM");
}

JNIEnv *usagi_vm_get_jni_env(UsagiVm *vm) {
  return usagi::catch_to_ptr<JNIEnv>(
      [&]() -> JNIEnv * {
        JNIEnv *env = usagi::current_env(vm);
        if (!env) {
          return nullptr;
        }

        usagi::clear_error();
        return env;
      },
      "failed to get JNIEnv");
}

jint usagi_vm_attach_current_thread(UsagiVm *vm, JNIEnv **out_env,
                                    int *out_attached) {
  if (out_env) {
    *out_env = nullptr;
  }
  if (out_attached) {
    *out_attached = 0;
  }

  return usagi::catch_to_status(
      [&]() -> jint {
        if (!usagi::valid_vm(vm) || !out_env) {
          usagi::set_error("invalid VM or output pointer");
          return JNI_ERR;
        }

        JavaVM *java_vm = vm->inner->GetJavaVM();
        if (!java_vm) {
          usagi::set_error("JavaVM unavailable");
          return JNI_ERR;
        }

        JNIEnv *env = nullptr;
        if (java_vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) ==
                JNI_OK &&
            env) {
          *out_env = env;
          if (out_attached) {
            *out_attached = 0;
          }
          usagi::clear_error();
          return JNI_OK;
        }

        const jint status = java_vm->AttachCurrentThread(&env, nullptr);
        if (status != JNI_OK || !env) {
          usagi::set_error("failed to attach current thread");
          return status == JNI_OK ? JNI_ERR : status;
        }

        *out_env = env;
        if (out_attached) {
          *out_attached = 1;
        }
        usagi::clear_error();
        return JNI_OK;
      },
      "unexpected error attaching thread", JNI_ERR);
}

jint usagi_vm_detach_current_thread(UsagiVm *vm) {
  return usagi::catch_to_status(
      [&]() -> jint {
        if (!usagi::valid_vm(vm)) {
          return JNI_ERR;
        }

        JavaVM *java_vm = vm->inner->GetJavaVM();
        if (!java_vm) {
          usagi::set_error("JavaVM unavailable");
          return JNI_ERR;
        }

        const jint status = java_vm->DetachCurrentThread();
        if (status != JNI_OK) {
          usagi::set_error("failed to detach current thread");
          return status;
        }

        usagi::clear_error();
        return JNI_OK;
      },
      "unexpected error detaching thread", JNI_ERR);
}

jint usagi_vm_generate_class_dump(UsagiVm *vm, const char *path) {
  return usagi::catch_to_status(
      [&]() -> jint {
        if (!usagi::valid_vm(vm) || !path || path[0] == '\0') {
          usagi::set_error("invalid VM or dump path");
          return JNI_ERR;
        }

#ifdef JNI_DEBUG
        vm->inner->GenerateClassDump(path);
        usagi::clear_error();
        return JNI_OK;
#else
        usagi::set_error("libjnivm was built without JNI_DEBUG");
        return JNI_ERR;
#endif
      },
      "failed to generate class dump", JNI_ERR);
}

UsagiObject *usagi_vm_find_class(UsagiVm *vm, const char *internal_name) {
  return usagi::catch_to_ptr<UsagiObject>(
      [&]() -> UsagiObject * {
        if (!usagi::valid_vm(vm) || !internal_name ||
            internal_name[0] == '\0') {
          usagi::set_error("invalid VM or class name");
          return nullptr;
        }

        JNIEnv *env = usagi::current_env(vm);
        if (!env) {
          return nullptr;
        }

        jclass local = env->FindClass(internal_name);
        if (!local) {
          usagi::take_exception(env);
          usagi::set_error("FindClass returned null");
          return nullptr;
        }

        usagi::LocalRefGuard guard{env, local};
        UsagiObject *object = usagi::make_object_handle(
            vm->inner, env, local, USAGI_OBJECT_KIND_CLASS);

        if (object) {
          usagi::clear_error();
        }
        return object;
      },
      "unexpected exception during class lookup");
}

} // extern "C"
