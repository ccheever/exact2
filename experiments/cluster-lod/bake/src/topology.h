// A simplification is usable only if it preserves the oriented patch boundary and
// introduces no nonmanifold edge. Reject its dependent coarser transitions as well.
#include <algorithm>
#include <tuple>
#include <utility>

static void preserveTopology(Output& o) {
    std::vector<uint32_t> remap(o.mesh.vertex_count);
    meshopt_generatePositionRemap(remap.data(), o.mesh.vertex_positions, o.mesh.vertex_count, o.mesh.vertex_positions_stride);
    std::vector<std::vector<size_t>> replacements(o.groups.size());
    for (size_t ci = 0; ci < o.clusters.size(); ++ci)
        if (o.clusters[ci].refined != UINT32_MAX)
            replacements[o.clusters[ci].refined].push_back(ci);
    using Edge = std::pair<uint64_t, int>;
    using Boundary = std::vector<std::tuple<uint64_t, size_t, int>>;
    auto boundary = [&](const std::vector<size_t>& clusters) {
        std::vector<Edge> edges;
        for (size_t ci : clusters) {
            const Cluster& c = o.clusters[ci];
            for (size_t ti = 0; ti < size_t(c.triangle_count) * 3; ti += 3) {
                uint32_t t[3];
                for (size_t k = 0; k < 3; ++k)
                    t[k] = remap[o.vertices[c.vertex_offset + o.indices[c.triangle_offset + ti + k]]];
                for (size_t k = 0; k < 3; ++k) {
                    uint32_t a = t[k], b = t[(k + 1) % 3];
                    edges.emplace_back((uint64_t(std::min(a,b)) << 32) | std::max(a,b), a < b ? 1 : -1);
                }
            }
        }
        std::sort(edges.begin(), edges.end());
        Boundary result;
        for (size_t i = 0; i < edges.size();) {
            size_t end = i;
            int winding = 0;
            while (end < edges.size() && edges[end].first == edges[i].first)
                winding += edges[end++].second;
            // Ordinary interior edges disappear; boundary and pre-existing scan
            // defects must survive unchanged, and new defects cannot be introduced.
            if (end - i != 2 || winding != 0)
                result.emplace_back(edges[i].first, end - i, winding);
            i = end;
        }
        return result;
    };
    std::vector<bool> stopped(o.groups.size());
    for (size_t gi = 0; gi < o.groups.size(); ++gi) {
        if (replacements[gi].empty()) continue;
        const Group& g = o.groups[gi];
        std::vector<size_t> source;
        for (size_t ci = g.first_cluster; ci < size_t(g.first_cluster) + g.cluster_count; ++ci)
            source.push_back(ci);
        stopped[gi] = boundary(source) != boundary(replacements[gi]);
    }
    // Callback IDs are topological. Once a group loses an invalid input cluster,
    // it cannot simplify as a whole; keep its remaining valid clusters terminal.
    for (size_t gi = 0; gi < o.groups.size(); ++gi) {
        const Group& g = o.groups[gi];
        for (size_t ci = g.first_cluster; ci < size_t(g.first_cluster) + g.cluster_count; ++ci) {
            uint32_t child = o.clusters[ci].refined;
            if (child != UINT32_MAX && stopped[child]) stopped[gi] = true;
        }
    }
    std::vector<Group> groups;
    std::vector<Cluster> clusters;
    std::vector<uint32_t> ids(o.groups.size(), UINT32_MAX);
    for (size_t gi = 0; gi < o.groups.size(); ++gi) {
        Group g = o.groups[gi];
        size_t first = clusters.size();
        if (stopped[gi]) g.simplified.error = FLT_MAX;
        for (size_t ci = g.first_cluster; ci < size_t(g.first_cluster) + g.cluster_count; ++ci) {
            Cluster c = o.clusters[ci];
            if (c.refined != UINT32_MAX && stopped[c.refined]) continue;
            c.group = narrow(groups.size());
            c.simplified = g.simplified;
            if (c.refined != UINT32_MAX) {
                c.refined = ids[c.refined];
                c.refined_bounds = groups[c.refined].simplified;
            }
            clusters.push_back(c);
        }
        if (clusters.size() == first) continue;
        ids[gi] = narrow(groups.size());
        g.first_cluster = narrow(first);
        g.cluster_count = narrow(clusters.size() - first);
        groups.push_back(g);
    }
    o.groups.swap(groups);
    o.clusters.swap(clusters);
    o.vendor_groups.clear();
    for (const Group& g : o.groups)
        o.vendor_groups.push_back({int(g.depth), g.simplified});
}
