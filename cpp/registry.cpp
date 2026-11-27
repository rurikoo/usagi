#include "internal.hpp"

#include <algorithm>
#include <type_traits>
#include <utility>

namespace {

bool valid_return_kind(char kind) noexcept {
  switch (kind) {
  case 'V':
  case 'Z':
  case 'B':
  case 'C':
  case 'S':
  case 'I':
  case 'J':
  case 'F':
  case 'D':
  case 'L':
  case '[':
    return true;
  default:
    return false;
  }
}

template <class T> constexpr bool matches_return_kind(char kind) noexcept {
  if constexpr (std::is_same_v<T, void>) {
    return kind == 'V';
  } else if constexpr (std::is_same_v<T, jboolean>) {
    return kind == 'Z';
  } else if constexpr (std::is_same_v<T, jbyte>) {
    return kind == 'B';
  } else if constexpr (std::is_same_v<T, jchar>) {
    return kind == 'C';
  } else if constexpr (std::is_same_v<T, jshort>) {
    return kind == 'S';
  } else if constexpr (std::is_same_v<T, jint>) {
    return kind == 'I';
  } else if constexpr (std::is_same_v<T, jlong>) {
    return kind == 'J';
  } else if constexpr (std::is_same_v<T, jfloat>) {
    return kind == 'F';
  } else if constexpr (std::is_same_v<T, jdouble>) {
    return kind == 'D';
  } else if constexpr (std::is_same_v<T, jobject>) {
    return kind == 'L' || kind == '[';
  } else {
    return false;
  }
}

template <class T> T jvalue_value(const jvalue &value) noexcept {
  if constexpr (std::is_same_v<T, void>) {
    return;
  } else if constexpr (std::is_same_v<T, jboolean>) {
    return value.z != JNI_FALSE ? JNI_TRUE : JNI_FALSE;
  } else if constexpr (std::is_same_v<T, jbyte>) {
    return value.b;
  } else if constexpr (std::is_same_v<T, jchar>) {
    return value.c;
  } else if constexpr (std::is_same_v<T, jshort>) {
    return value.s;
  } else if constexpr (std::is_same_v<T, jint>) {
    return value.i;
  } else if constexpr (std::is_same_v<T, jlong>) {
    return value.j;
  } else if constexpr (std::is_same_v<T, jfloat>) {
    return value.f;
  } else if constexpr (std::is_same_v<T, jdouble>) {
    return value.d;
  } else if constexpr (std::is_same_v<T, jobject>) {
    return value.l;
  }
}

void throw_java_exception(jnivm::ENV *jenv,
                          const std::string &message) noexcept {
  if (!jenv) {
    return;
  }

  try {
    JNIEnv *env = jenv->GetJNIEnv();
    if (!env) {
      return;
    }

    jclass ex_cls = env->FindClass("java/lang/RuntimeException");
    if (!ex_cls) {
      return;
    }

    usagi::LocalRefGuard guard{env, ex_cls};
    env->ThrowNew(ex_cls, message.empty() ? "usagi method callback failed"
                                          : message.c_str());
  } catch (...) {
  }
}

class UsagiMethodHandle : public jnivm::MethodHandle {
public:
  UsagiMethodHandle(bool is_static, char return_kind, void *user_data,
                    UsagiMethodCallback callback, UsagiMethodDrop drop)
      : is_static_(is_static), return_kind_(return_kind), user_data_(user_data),
        callback_(callback), drop_(drop) {}

  ~UsagiMethodHandle() {
    if (drop_) {
      drop_(user_data_);
    }
  }

#define USAGI_DEFINE_INVOKE(type)                                              \
  type StaticInvoke(jnivm::ENV *jenv, jnivm::Class *, const jvalue *values,    \
                    jnivm::impl::MethodHandleBase<type>) override {            \
    return invoke<type>(jenv, true, values);                                   \
  }                                                                            \
  type InstanceInvoke(jnivm::ENV *jenv, jobject, const jvalue *values,         \
                      jnivm::impl::MethodHandleBase<type>) override {          \
    return invoke<type>(jenv, false, values);                                  \
  }                                                                            \
  type NonVirtualInstanceInvoke(                                               \
      jnivm::ENV *jenv, jobject, const jvalue *values,                         \
      jnivm::impl::MethodHandleBase<type>) override {                          \
    return invoke<type>(jenv, false, values);                                  \
  }

  USAGI_DEFINE_INVOKE(jobject)
  USAGI_DEFINE_INVOKE(jboolean)
  USAGI_DEFINE_INVOKE(jbyte)
  USAGI_DEFINE_INVOKE(jchar)
  USAGI_DEFINE_INVOKE(jshort)
  USAGI_DEFINE_INVOKE(jint)
  USAGI_DEFINE_INVOKE(jlong)
  USAGI_DEFINE_INVOKE(jfloat)
  USAGI_DEFINE_INVOKE(jdouble)
  USAGI_DEFINE_INVOKE(void)

#undef USAGI_DEFINE_INVOKE

private:
  template <class T> T default_value() noexcept {
    if constexpr (std::is_void_v<T>) {
      return;
    } else {
      return T{};
    }
  }

  template <class T>
  T invoke(jnivm::ENV *jenv, bool called_static, const jvalue *values) {
    if (called_static != is_static_) {
      usagi::set_error(is_static_
                           ? "static method invoked as an instance method"
                           : "instance method invoked as a static method");
      throw_java_exception(jenv, usagi::current_error());
      return default_value<T>();
    }

    if (!matches_return_kind<T>(return_kind_)) {
      usagi::set_error("method invoked with mismatched JNI return type");
      throw_java_exception(jenv, usagi::current_error());
      return default_value<T>();
    }

    JNIEnv *env = jenv ? jenv->GetJNIEnv() : nullptr;
    if (!env) {
      usagi::set_error("JNIEnv unavailable in callback");
      throw_java_exception(jenv, usagi::current_error());
      return default_value<T>();
    }

    jvalue out{};
    if (callback_(user_data_, env, values, &out) != USAGI_OK) {
      if (usagi::current_error().empty()) {
        usagi::set_error("usagi method callback failed");
      }
      throw_java_exception(jenv, usagi::current_error());
      return default_value<T>();
    }

    return jvalue_value<T>(out);
  }

  bool is_static_;
  char return_kind_;
  void *user_data_;
  UsagiMethodCallback callback_;
  UsagiMethodDrop drop_;
};

struct CallbackOwner {
  void *data;
  UsagiMethodDrop drop;
  bool released = false;

  ~CallbackOwner() {
    if (!released && drop) {
      drop(data);
    }
  }

  void release() noexcept { released = true; }
};

} // namespace

extern "C" {

jint usagi_class_register_method(UsagiObject *class_object, const char *name,
                                 const char *signature, int is_static,
                                 char return_kind, void *user_data,
                                 UsagiMethodCallback callback,
                                 UsagiMethodDrop drop) {
  CallbackOwner owner{user_data, drop};

  return usagi::catch_to_status(
      [&]() -> jint {
        if (!callback) {
          usagi::set_error("null method callback");
          return USAGI_ERROR;
        }

        if (!usagi::valid_object(class_object)) {
          return USAGI_ERROR;
        }

        if (class_object->kind != USAGI_OBJECT_KIND_CLASS) {
          usagi::set_error("object is not a Java class");
          return USAGI_ERROR;
        }

        if (!name || name[0] == '\0' || !signature || signature[0] == '\0') {
          usagi::set_error("invalid method name or signature");
          return USAGI_ERROR;
        }

        if (!valid_return_kind(return_kind)) {
          usagi::set_error("invalid method return kind");
          return USAGI_ERROR;
        }

        JNIEnv *env = usagi::current_env(class_object->owner);
        if (!env) {
          return USAGI_ERROR;
        }

        auto *jenv = jnivm::ENV::FromJNIEnv(env);
        std::shared_ptr<jnivm::Class> cl =
            jnivm::JNITypes<std::shared_ptr<jnivm::Class>>::JNICast(
                jenv, static_cast<jclass>(class_object->global_ref));

        if (!cl) {
          usagi::set_error("jnivm could not provide the Java class");
          return USAGI_ERROR;
        }

        auto method = std::make_shared<jnivm::Method>();
        method->name = name;
        method->signature = signature;
        method->_static = is_static != 0;
        method->native = nullptr;
        method->nativehandle = std::make_shared<UsagiMethodHandle>(
            method->_static, return_kind, user_data, callback, drop);
        owner.release();

        std::shared_ptr<jnivm::MethodHandle> old_handle;
        {
          std::lock_guard<std::mutex> lock(cl->mtx);
          auto existing = std::find_if(
              cl->methods.begin(), cl->methods.end(),
              [&method](const std::shared_ptr<jnivm::Method> &candidate) {
                return candidate->_static == method->_static &&
                       candidate->name == method->name &&
                       candidate->signature == method->signature;
              });

          if (existing != cl->methods.end()) {
            old_handle = std::move((*existing)->nativehandle);
            (*existing)->nativehandle = method->nativehandle;
            (*existing)->native = nullptr;
          } else {
            cl->methods.push_back(method);
          }
        }

        usagi::clear_error();
        return USAGI_OK;
      },
      "unexpected exception registering method");
}

} // extern "C"
