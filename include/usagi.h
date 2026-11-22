#ifndef USAGI_H
#define USAGI_H

#include <jni.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct UsagiVm UsagiVm;
typedef struct UsagiObject UsagiObject;

enum {
  USAGI_OK = 0,
  USAGI_ERROR = -1,
  USAGI_BUFFER_TOO_SMALL = -2,
  USAGI_EXCEPTION = -3
};

typedef enum UsagiObjectKind {
  USAGI_OBJECT_KIND_OBJECT = 0,
  USAGI_OBJECT_KIND_CLASS = 1,
  USAGI_OBJECT_KIND_STRING = 2
} UsagiObjectKind;

/* create and destroy a VM */
UsagiVm *usagi_vm_create(void);
void usagi_vm_destroy(UsagiVm *vm);

/* access the JNI interfaces */
JavaVM *usagi_vm_get_java_vm(UsagiVm *vm);
JNIEnv *usagi_vm_get_jni_env(UsagiVm *vm);

/* thread lifecycle; out_attached is set to 1 only if this call attached the thread */
jint usagi_vm_attach_current_thread(UsagiVm *vm, JNIEnv **out_env,
                                    int *out_attached);

jint usagi_vm_detach_current_thread(UsagiVm *vm);

/* generate diagnostic stubs when libjnivm debug flag is enabled */
jint usagi_vm_generate_class_dump(UsagiVm *vm, const char *path);

/* class lookup returns an owned object handle */
UsagiObject *usagi_vm_find_class(UsagiVm *vm, const char *internal_name);

UsagiObject *usagi_vm_new_string_utf16(UsagiVm *vm, const uint16_t *units,
                                       size_t length);

/* releases the reference and the handle */
void usagi_object_destroy(UsagiObject *object);

jint usagi_string_utf16_length(const UsagiObject *object, size_t *out_length);

jint usagi_string_copy_utf16(const UsagiObject *object, uint16_t *buffer,
                             size_t capacity, size_t *out_length);

/* jmethodID, or NULL on failure */
void *usagi_class_get_method(const UsagiObject *class_object, const char *name,
                             const char *signature, int is_static);

/* return_kind: JNI type char of the return type
   out_object_kind: UsagiObjectKind for L results */
jint usagi_call_method(const UsagiObject *class_object,
                       const UsagiObject *receiver, void *method_id,
                       int is_static, char return_kind, jvalue *args,
                       jvalue *out_value, int out_object_kind,
                       UsagiObject **out_object);

/* Rust callback receiving arguments and writing the return value */
typedef jint (*UsagiMethodCallback)(void *user_data, const jvalue *args,
                                    jvalue *out_value);

/* frees user_data; called when the method is replaced or the VM is destroyed */
typedef void (*UsagiMethodDrop)(void *user_data);

/* registers or replaces a method; takes ownership of user_data on every path */
jint usagi_class_register_method(UsagiObject *class_object, const char *name,
                                 const char *signature, int is_static,
                                 char return_kind, void *user_data,
                                 UsagiMethodCallback callback,
                                 UsagiMethodDrop drop);

/* sets the thread-local native error message, used by Rust callbacks */
void usagi_set_error(const char *message);

/* error from the last failed call on this thread */
const char *usagi_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* USAGI_H */
