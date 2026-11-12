#include "scheme.h"
#include <cstdio>
int main(int argc, char **argv) {
    if (argc != 2) return 1;
    auto file = std::fopen(argv[1], "wb");
    if (!file) return 2;
    for (unsigned count : {0U, 1U, 2U}) {
        mtpBuffer buffer;
        MTPVector<MTPint>(MTP_vector<MTPint>(count, MTP_int(-1234567))).write(buffer);
        const auto size = uint32_t(buffer.size() * 4);
        std::fwrite(&size, 4, 1, file);
        std::fwrite(buffer.data(), 1, size, file);
    }
    std::fclose(file);
}
