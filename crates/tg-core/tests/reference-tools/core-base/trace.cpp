#include "storage/storage_sparse_ids_list.h"
#include "storage/storage_shared_media.h"
#include "storage/storage_user_photos.h"
#include <sstream>

using namespace Storage;

std::string opt(std::optional<int> value) {
    return value ? std::to_string(*value) : "-";
}
template <typename Range>
void ids(const Range &values) {
    std::cout << ' ' << values.size();
    for (auto value : values) std::cout << ' ' << value;
    std::cout << '\n';
}
void result(const SparseIdsListResult &value) {
    std::cout << "R " << opt(value.count) << ' '
        << opt(value.skippedBefore) << ' ' << opt(value.skippedAfter);
    ids(value.messageIds);
}
void update(const SparseIdsSliceUpdate &value) {
    std::cout << value.range.from << ' ' << value.range.till << ' '
        << opt(value.count) << ' ' << (value.messages != nullptr);
    ids(value.messages ? *value.messages : base::flat_set<MsgId>());
}
SharedMediaTypesMask mask(unsigned bits) {
    if (bits == std::numeric_limits<unsigned>::max()) return SharedMediaTypesMask::All();
    auto result = SharedMediaTypesMask();
    for (auto i = 0; i < kSharedMediaTypeCount; ++i) {
        if (bits & (1U << i)) result.set(static_cast<SharedMediaType>(i));
    }
    return result;
}

int main() {
    SparseIdsList sparse;
    Facade facade;
    auto &media = facade;
    auto &photos = facade;
    rpl::lifetime life;
    sparse.sliceUpdated() | rpl::on_next([](const auto &value) {
        std::cout << "S "; update(value);
    }, life);
    media.sharedMediaSliceUpdated() | rpl::on_next([](const auto &value) {
        std::cout << "M " << value.peerId << ' ' << value.topicRootId << ' '
            << value.monoforumPeerId << ' ' << int(value.type) << ' ';
        update(value.data);
    }, life);
    media.sharedMediaOneRemoved() | rpl::on_next([](const auto &value) {
        std::cout << "O " << value.peerId << ' ' << value.topicRootId << ' '
            << value.monoforumPeerId << ' ' << value.messageId << '\n';
    }, life);
    media.sharedMediaAllRemoved() | rpl::on_next([](const auto &value) {
        std::cout << "A " << value.peerId << ' ' << value.topicRootId << ' '
            << value.monoforumPeerId << '\n';
    }, life);
    media.sharedMediaBottomInvalidated() | rpl::on_next([](const auto &value) {
        std::cout << "I " << value.peerId << '\n';
    }, life);
    photos.userPhotosSliceUpdated() | rpl::on_next([](const auto &value) {
        std::cout << "P " << value.userId << ' ' << opt(value.count);
        ids(*value.photoIds);
    }, life);
    std::string line;
    auto index = 0;
    while (std::getline(std::cin, line)) {
        std::cout << "@ " << index++ << '\n';
        std::istringstream input(line);
        std::string command;
        input >> command;
        if (command == "ss" || command == "ms") {
            PeerId peer = 0, mono = 0;
            MsgId topic = 0;
            auto type = 0;
            if (command == "ms") input >> peer >> topic >> mono >> type;
            MsgRange range;
            int count, size;
            input >> range.from >> range.till >> count >> size;
            std::vector<MsgId> values(size);
            for (auto &value : values) input >> value;
            const auto total = count == -1 ? std::nullopt : std::optional(count);
            if (command == "ss") sparse.addSlice(std::move(values), range, total);
            else media.add(SharedMediaAddSlice(peer, topic, mono,
                static_cast<SharedMediaType>(type), std::move(values), range, total));
        } else if (command == "sn" || command == "sr" || command == "se") {
            MsgId id; input >> id;
            if (command == "sn") sparse.addNew(id);
            else if (command == "sr") sparse.removeOne(id);
            else { MsgRange range; input >> range.from >> range.till; sparse.addExisting(id, range); }
        } else if (command == "sa") sparse.removeAll();
        else if (command == "si") sparse.invalidateBottom();
        else if (command == "sq") {
            MsgId id; int before, after; input >> id >> before >> after;
            sparse.query({id, before, after}) | rpl::on_next_done(result, [] { std::cout << "D\n"; }, life);
            std::cout << "E " << sparse.empty() << '\n';
        } else if (command == "sc") {
            MsgId id; int limit, divisor; input >> id >> limit >> divisor;
            auto calls = 0;
            const auto count = sparse.countAfter(id, limit, [&](MsgId value) {
                ++calls; return value % divisor == 0;
            });
            std::cout << "C " << opt(count) << ' ' << calls << '\n';
        } else if (command == "mn" || command == "me" || command == "mr") {
            PeerId peer, mono; MsgId topic, id; unsigned bits;
            input >> peer >> topic >> mono >> bits >> id;
            if (command == "mn") media.add(SharedMediaAddNew(peer, topic, mono, mask(bits), id));
            else if (command == "mr") media.remove(SharedMediaRemoveOne(peer, topic, mono, mask(bits), id));
            else { MsgRange range; input >> range.from >> range.till;
                media.add(SharedMediaAddExisting(peer, topic, mono, mask(bits), id, range)); }
        } else if (command == "ma" || command == "mu") {
            PeerId peer, mono; MsgId topic; input >> peer >> topic >> mono;
            if (command == "mu") media.unload({peer, topic, mono});
            else { unsigned bits; input >> bits;
                auto query = SharedMediaRemoveAll(peer, mask(bits));
                query.topicRootId = topic; query.monoforumPeerId = mono;
                media.remove(std::move(query)); }
        } else if (command == "mi") {
            PeerId peer; input >> peer; media.invalidate({peer});
        } else if (command == "mq") {
            PeerId peer, mono; MsgId topic, id; int type, before, after;
            input >> peer >> topic >> mono >> type >> id >> before >> after;
            const auto key = SharedMediaKey(peer, topic, mono, static_cast<SharedMediaType>(type), id);
            result(media.snapshot({key, before, after}));
            media.query({key, before, after}) | rpl::on_next_done(result, [] { std::cout << "D\n"; }, life);
            std::cout << "E " << media.empty(key) << '\n';
        } else if (command == "ps") {
            UserId user; int count, size; input >> user >> count >> size;
            std::vector<PhotoId> values(size);
            for (auto &value : values) input >> value;
            photos.add(UserPhotosAddSlice(user, std::move(values), count));
        } else if (command == "pq") {
            UserId user; PhotoId id; int back, before, after;
            input >> user >> id >> back >> before >> after;
            auto key = UserPhotosKey(user, id); key.back = back;
            photos.query({key, before, after}) | rpl::on_next_done([](const auto &value) {
                std::cout << "R " << opt(value.count) << ' ' << opt(value.skippedBefore)
                    << ' ' << value.skippedAfter; ids(value.photoIds);
            }, [] { std::cout << "D\n"; }, life);
        } else if (command == "pn" || command == "pb" || command == "pr" || command == "pa" || command == "px") {
            UserId user; PhotoId id; input >> user >> id;
            if (command == "pn") photos.add(UserPhotosAddNew(user, id));
            else if (command == "pb") photos.add(UserPhotosSetBack(user, id));
            else if (command == "pr") photos.remove(UserPhotosRemoveOne(user, id));
            else if (command == "pa") photos.remove(UserPhotosRemoveAfter(user, id));
            else { PhotoId replacement; input >> replacement; photos.replace(UserPhotosReplace(user, id, replacement)); }
        } else { std::cerr << "Unknown command: " << command; return 1; }
        if (!input) { std::cerr << "Invalid input: " << line; return 1; }
    }
}
