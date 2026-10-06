Markdown/native collection integration (2026-09-18): the retained reader's
explicit row-height list and upstream `virtualized` collection both build.
Carry logical selection/full-document copy to the collection path, compare
its full-document workload, then retire the redundant measured-list path.
Do not infer a performance gain merely from combining the branches.
