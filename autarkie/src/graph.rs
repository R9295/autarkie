use petgraph::graphmap::GraphMap;
use petgraph::visit::IntoNeighbors;
use petgraph::Directed;
use std::collections::HashSet;
use std::fmt::Debug;

pub fn find_cycles<N: Copy + Eq + std::hash::Hash + Ord + Debug, E>(
    graph: &GraphMap<N, E, Directed>,
) -> HashSet<Vec<N>> {
    let mut cycles = HashSet::new();
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let mut done = HashSet::new();
    for node in graph.nodes() {
        visited.drain();
        if !done.contains(&node) {
            dfs_cycle(graph, node, &mut visited, &mut stack, &mut cycles);
        }
        done.insert(node);
    }

    cycles
}

pub fn dfs_cycle<N: Copy + Eq + std::hash::Hash + Ord + Debug, E>(
    graph: &GraphMap<N, E, Directed>,
    node: N,
    visited: &mut HashSet<N>,
    stack: &mut Vec<N>,
    cycles: &mut HashSet<Vec<N>>,
) {
    visited.insert(node);
    stack.push(node);
    for neighbor in graph.neighbors(node) {
        if !visited.contains(&neighbor) {
            dfs_cycle(graph, neighbor, visited, stack, cycles);
        } else if stack.contains(&neighbor) {
            let cycle_start = stack.iter().position(|&x| x == neighbor).unwrap();
            let cycle = stack[cycle_start..].to_vec();
            cycles.insert(cycle);
        }
    }

    stack.pop();
}
