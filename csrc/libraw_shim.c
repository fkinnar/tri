/* Accès aux champs de libraw_data_t.params qui n'ont pas de setter dans l'API C.
 * Compilé avec le vrai libraw.h : le layout de la struct est toujours le bon. */
#include <libraw/libraw.h>

void tri_libraw_set_use_camera_wb(libraw_data_t *lr, int value) {
    lr->params.use_camera_wb = value;
}

void tri_libraw_set_half_size(libraw_data_t *lr, int value) {
    lr->params.half_size = value;
}

/* Orientation demandée par le boîtier (0, 3 = 180°, 5 = 90° anti-horaire, 6 = 90° horaire). */
int tri_libraw_get_flip(libraw_data_t *lr) {
    return lr->sizes.flip;
}

/* Dimensions de l'image développée, avant rotation et avant half_size. */
void tri_libraw_get_size(libraw_data_t *lr, int *width, int *height) {
    *width = lr->sizes.width;
    *height = lr->sizes.height;
}

/* Données brutes du capteur après libraw_unpack, pour l'histogramme RAW. Les niveaux de
 * noir et de blanc sont lus dans rawdata.color, la copie faite par unpack (certains
 * boîtiers ne les connaissent qu'après lecture des pixels masqués). */
typedef struct {
    const unsigned short *raw_image; /* NULL : pas de mosaïque (DNG linéaire, Foveon) */
    unsigned raw_width, raw_height, raw_pitch; /* raw_pitch en octets */
    unsigned left_margin, top_margin, width, height; /* zone visible */
    unsigned black, maximum;
    const unsigned *cblack; /* tableau complet de LibRaw : [0..3] par canal, [4] et [5]
                               dimensions du motif de noir, [6..] le motif (Fuji : 6 × 6) */
    unsigned cblack_len;
    unsigned filters; /* 0 : pas de mosaïque ; 9 : X-Trans ; sinon motif Bayer */
} tri_raw_info;

void tri_libraw_raw_info(libraw_data_t *lr, tri_raw_info *out) {
    out->raw_image = lr->rawdata.raw_image;
    out->raw_width = lr->sizes.raw_width;
    out->raw_height = lr->sizes.raw_height;
    out->raw_pitch = lr->sizes.raw_pitch;
    out->left_margin = lr->sizes.left_margin;
    out->top_margin = lr->sizes.top_margin;
    out->width = lr->sizes.width;
    out->height = lr->sizes.height;
    out->black = lr->rawdata.color.black;
    out->cblack = lr->rawdata.color.cblack;
    out->cblack_len = LIBRAW_CBLACK_SIZE;
    out->maximum = lr->rawdata.color.maximum;
    out->filters = lr->idata.filters;
}

/* Métadonnées de prise de vue lues par LibRaw (open_file suffit, pas d'unpack). La date
 * est mise en forme ici, en heure locale, comme LibRaw l'a lue (mktime). */
typedef struct {
    char make[64], model[64], lens[128];
    float iso, shutter, aperture, focal, focal_35mm;
    char date[32]; /* "AAAA-MM-JJ HH:MM:SS", vide si inconnue */
    int width, height; /* zone visible, avant rotation */
} tri_metadata;

#include <string.h>
#include <time.h>

void tri_libraw_metadata(libraw_data_t *lr, tri_metadata *out) {
    const char *lens;
    memset(out, 0, sizeof(*out));
    strncpy(out->make, lr->idata.make, sizeof(out->make) - 1);
    strncpy(out->model, lr->idata.model, sizeof(out->model) - 1);
    lens = lr->lens.Lens[0] ? lr->lens.Lens : lr->lens.makernotes.Lens;
    strncpy(out->lens, lens, sizeof(out->lens) - 1);
    out->iso = lr->other.iso_speed;
    out->shutter = lr->other.shutter;
    out->aperture = lr->other.aperture;
    out->focal = lr->other.focal_len;
    out->focal_35mm = lr->lens.FocalLengthIn35mmFormat;
    if (lr->other.timestamp > 0) {
        time_t t = lr->other.timestamp;
        struct tm *local = localtime(&t);
        if (local)
            strftime(out->date, sizeof(out->date), "%Y-%m-%d %H:%M:%S", local);
    }
    out->width = lr->sizes.width;
    out->height = lr->sizes.height;
}
