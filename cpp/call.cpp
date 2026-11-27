#include "internal.hpp"

extern "C" {

void *usagi_class_get_method(const UsagiObject *class_object, const char *name,
                             const char *signature, int is_static) {
  return usagi::catch_to_ptr<void>(
      [&]() -> void * {
        if (!usagi::valid_object(class_object)) {
          return nullptr;
        }

        if (class_object->kind != USAGI_OBJECT_KIND_CLASS) {
          usagi::set_error("object is not a Java class");
          return nullptr;
        }

        if (!name || name[0] == '\0' || !signature || signature[0] == '\0') {
          usagi::set_error("invalid method name or signature");
          return nullptr;
        }

        JNIEnv *env = usagi::current_env(class_object->owner);
        if (!env) {
          return nullptr;
        }

        jclass cls = static_cast<jclass>(class_object->global_ref);
        jmethodID id = is_static ? env->GetStaticMethodID(cls, name, signature)
                                 : env->GetMethodID(cls, name, signature);

        if (!id) {
          usagi::take_exception(env);
          usagi::set_error("method lookup returned null");
          return nullptr;
        }

        usagi::clear_error();
        return reinterpret_cast<void *>(id);
      },
      "unexpected exception during method lookup");
}

jint usagi_call_method(const UsagiObject *class_object,
                       const UsagiObject *receiver, void *method_id,
                       int is_static, char return_kind, jvalue *args,
                       jvalue *out_value, int out_object_kind,
                       UsagiObject **out_object) {
  if (!out_value || !out_object) {
    usagi::set_error("null output pointer");
    return USAGI_ERROR;
  }

  *out_object = nullptr;
  out_value->j = 0;

  return usagi::catch_to_status(
      [&]() -> jint {
        if (!usagi::valid_object(class_object)) {
          return USAGI_ERROR;
        }

        if (!method_id) {
          usagi::set_error("null method id");
          return USAGI_ERROR;
        }

        if (class_object->kind != USAGI_OBJECT_KIND_CLASS) {
          usagi::set_error("object is not a Java class");
          return USAGI_ERROR;
        }

        if (!usagi::valid_object_kind(out_object_kind)) {
          usagi::set_error("invalid output object kind");
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(class_object->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        jclass cls = static_cast<jclass>(class_object->global_ref);
        jmethodID mid = static_cast<jmethodID>(method_id);
        jobject self = nullptr;

        if (!is_static) {
          if (!usagi::valid_object(receiver)) {
            return USAGI_ERROR;
          }

          if (receiver->owner != class_object->owner) {
            usagi::set_error("receiver belongs to a different vm");
            return USAGI_ERROR;
          }

          if (!env->IsInstanceOf(receiver->global_ref, cls)) {
            usagi::set_error(
                "receiver is not an instance of the method's class");
            return USAGI_ERROR;
          }

          self = receiver->global_ref;
        }

        jobject result = nullptr;
        bool threw = false;
        std::string what;

        try {
          switch (return_kind) {
          case 'V':
            if (is_static) {
              env->CallStaticVoidMethodA(cls, mid, args);
            } else {
              env->CallVoidMethodA(self, mid, args);
            }
            break;
          case 'Z': {
            const jboolean raw =
                is_static ? env->CallStaticBooleanMethodA(cls, mid, args)
                          : env->CallBooleanMethodA(self, mid, args);
            out_value->z = raw != JNI_FALSE ? JNI_TRUE : JNI_FALSE;
            break;
          }
          case 'B':
            out_value->b = is_static
                               ? env->CallStaticByteMethodA(cls, mid, args)
                               : env->CallByteMethodA(self, mid, args);
            break;
          case 'C':
            out_value->c = is_static
                               ? env->CallStaticCharMethodA(cls, mid, args)
                               : env->CallCharMethodA(self, mid, args);
            break;
          case 'S':
            out_value->s = is_static
                               ? env->CallStaticShortMethodA(cls, mid, args)
                               : env->CallShortMethodA(self, mid, args);
            break;
          case 'I':
            out_value->i = is_static ? env->CallStaticIntMethodA(cls, mid, args)
                                     : env->CallIntMethodA(self, mid, args);
            break;
          case 'J':
            out_value->j = is_static
                               ? env->CallStaticLongMethodA(cls, mid, args)
                               : env->CallLongMethodA(self, mid, args);
            break;
          case 'F':
            out_value->f = is_static
                               ? env->CallStaticFloatMethodA(cls, mid, args)
                               : env->CallFloatMethodA(self, mid, args);
            break;
          case 'D':
            out_value->d = is_static
                               ? env->CallStaticDoubleMethodA(cls, mid, args)
                               : env->CallDoubleMethodA(self, mid, args);
            break;
          case 'L':
          case '[':
            result = is_static ? env->CallStaticObjectMethodA(cls, mid, args)
                               : env->CallObjectMethodA(self, mid, args);
            break;
          default:
            usagi::set_error("unknown return kind");
            return USAGI_ERROR;
          }
        } catch (const std::exception &e) {
          threw = true;
          what = e.what();
        } catch (...) {
          threw = true;
        }

        usagi::LocalRefGuard guard{env, result};
        const bool pending = usagi::take_exception(env);

        if (threw || pending) {
          out_value->j = 0;
          if (!what.empty()) {
            usagi::set_error(what.c_str());
          } else if (usagi::current_error().empty()) {
            usagi::set_error("java method threw an exception");
          }
          return USAGI_EXCEPTION;
        }

        if (result) {
          const auto kind = static_cast<UsagiObjectKind>(out_object_kind);
          UsagiObject *handle =
              usagi::make_object_handle(class_object->owner, env, result, kind);
          if (!handle) {
            return USAGI_ERROR;
          }

          *out_object = handle;
        }

        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception calling Java method");
}

} // extern "C"
