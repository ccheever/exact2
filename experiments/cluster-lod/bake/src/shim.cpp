#include "meshoptimizer.h"
#define CLUSTERLOD_IMPLEMENTATION
#include "clusterlod.h"
#include <cstdint>
#include <memory>
#include <stdexcept>
#include <climits>

static uint32_t narrow(size_t count) {
    if (count > UINT32_MAX) throw std::overflow_error("cluster count/offset exceeds uint32");
    return static_cast<uint32_t>(count);
}

struct Config {
    uint32_t max_triangles, page_bytes, partition_size, vertex_encoding;
    float normal_weight, color_weight, simplify_ratio, simplify_threshold;
    float error_merge_previous, error_merge_additive;
    uint32_t reserved[2];
};
struct Cluster {
    float sphere[4];
    clodBounds simplified, refined_bounds;
    float cone_apex[3], cone_cutoff, cone_axis[3];
    uint32_t group, refined, page, vertex_offset, vertex_count, triangle_offset, triangle_count, depth, reserved[3];
};
struct Group {
    clodBounds simplified;
    uint32_t depth, first_cluster, cluster_count;
};
template<class G> constexpr size_t hierarchyLevels(const G* groups, size_t count) {
    size_t levels=0;
    for(size_t i=0;i<count;i++) levels=std::max(levels,size_t(groups[i].depth)+1);
    return levels;
}
struct DepthFixture { uint32_t depth; };
constexpr DepthFixture stoppedDepths[] = {{0},{4},{1}};
static_assert(hierarchyLevels(stoppedDepths,3)==5, "topology can leave depths out of order");
static_assert(sizeof(Cluster) == 128, "cluster ABI");
static_assert(sizeof(Group) == 32, "group ABI");
static_assert(sizeof(clodNode) == 32, "node ABI");
static_assert(sizeof(Config) == 48, "config ABI");
struct Output {
    std::vector<Cluster> clusters;
    std::vector<Group> groups;
    std::vector<clodGroup> vendor_groups;
    std::vector<clodNode> nodes;
    std::vector<uint32_t> vertices;
    std::vector<uint8_t> indices;
    clodMesh mesh;
    uint32_t transitions_checked = 0, transitions_rejected = 0, transitions_stopped = 0;
};
struct View {
    const Cluster* clusters; size_t cluster_count;
    const Group* groups; size_t group_count;
    const clodNode* nodes; size_t node_count;
    const uint32_t* vertices; size_t vertex_count;
    const uint8_t* indices; size_t index_count;
    uint32_t transitions_checked, transitions_rejected, transitions_stopped;
    void* owner;
};
#include "topology.h"
static int emit(void* context, clodGroup group, const clodCluster* clusters, size_t count) {
    Output& o = *static_cast<Output*>(context);
    group.simplified.error = std::max(group.simplified.error, FLT_MIN);
    if (o.groups.size() >= INT_MAX || group.depth < 0 || count > UINT32_MAX - o.clusters.size())
        throw std::overflow_error("group/cluster id overflow");
    int id = static_cast<int>(o.groups.size());
    o.groups.push_back({group.simplified, narrow(group.depth), narrow(o.clusters.size()), narrow(count)});
    o.vendor_groups.push_back(group);
    for (size_t i = 0; i < count; ++i) {
        const clodCluster& c = clusters[i];
        Cluster result = {};
        meshopt_Bounds b = meshopt_computeClusterBounds(c.indices, c.index_count, o.mesh.vertex_positions, o.mesh.vertex_count, o.mesh.vertex_positions_stride);
        memcpy(result.sphere, b.center, 12); result.sphere[3] = b.radius;
        memcpy(result.cone_apex, b.cone_apex, 12);
        memcpy(result.cone_axis, b.cone_axis, 12); result.cone_cutoff = b.cone_cutoff;
        result.simplified = group.simplified;
        result.refined = uint32_t(c.refined);
        if (c.refined >= id || c.refined < -1 || c.vertex_count > UINT32_MAX - o.vertices.size()
            || c.index_count > UINT32_MAX - o.indices.size()) throw std::overflow_error("geometry offset/refinement overflow");
        if (c.refined >= 0) result.refined_bounds = o.groups[c.refined].simplified;
        result.group = narrow(id); result.depth = narrow(group.depth);
        result.vertex_offset = narrow(o.vertices.size()); result.vertex_count = narrow(c.vertex_count);
        result.triangle_offset = narrow(o.indices.size()); result.triangle_count = narrow(c.index_count / 3);
        o.vertices.resize(o.vertices.size() + c.vertex_count);
        o.indices.resize(o.indices.size() + c.index_count);
        clodLocalIndices(o.vertices.data() + result.vertex_offset, o.indices.data() + result.triangle_offset, c.indices, c.index_count);
        o.clusters.push_back(result);
    }
    return id;
}
extern "C" bool exact_clod_build(const float* positions, size_t vertex_count, const uint32_t* indices, size_t index_count, const float* attributes, bool colors, Config config, View* view) {
    try {
        auto output = std::make_unique<Output>();
        clodConfig c = clodDefaultConfig(config.max_triangles);
        c.partition_size = config.partition_size;
        c.simplify_permissive = false;
        c.simplify_fallback_permissive = false;
        c.simplify_fallback_sloppy = false;
        c.simplify_ratio = config.simplify_ratio;
        c.simplify_threshold = config.simplify_threshold;
        c.simplify_error_merge_previous = config.error_merge_previous;
        c.simplify_error_merge_additive = config.error_merge_additive;
        float weights[7] = {config.normal_weight, config.normal_weight, config.normal_weight, config.color_weight, config.color_weight, config.color_weight, config.color_weight};
        output->mesh = {indices, index_count, vertex_count, positions, 12, attributes, 28, nullptr, weights, colors ? size_t(7) : size_t(3), 0};
        clodBuild(c, output->mesh, output.get(), emit);
        preserveTopology(*output);
        output->mesh = {}; // All input pointers were borrowed only for synchronous construction.
        if (output->groups.empty()) return false;
        size_t levels = hierarchyLevels(output->groups.data(), output->groups.size());
        output->nodes.resize(narrow(clodBuildHierarchyBound(output->groups.size(), 8, levels)));
        output->nodes.resize(clodBuildHierarchy(output->nodes.data(), output->vendor_groups.data(), output->groups.size(), 8, levels));
        *view = {output->clusters.data(), output->clusters.size(), output->groups.data(), output->groups.size(), output->nodes.data(), output->nodes.size(), output->vertices.data(), output->vertices.size(), output->indices.data(), output->indices.size(), output->transitions_checked, output->transitions_rejected, output->transitions_stopped, output.get()};
        output.release();
        return true;
    } catch (...) { return false; }
}
extern "C" void exact_clod_free(void* owner) { delete static_cast<Output*>(owner); }
