/* Accès aux champs de libraw_data_t.params qui n'ont pas de setter dans l'API C.
 * Compilé avec le vrai libraw.h : le layout de la struct est toujours le bon. */
#include <libraw/libraw.h>

void tri_libraw_set_use_camera_wb(libraw_data_t *lr, int value) {
    lr->params.use_camera_wb = value;
}

void tri_libraw_set_half_size(libraw_data_t *lr, int value) {
    lr->params.half_size = value;
}
