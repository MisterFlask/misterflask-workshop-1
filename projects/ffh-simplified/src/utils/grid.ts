import type { Coord, Tile } from '../types';

// Direction vectors for 4-way movement (no diagonals)
export const DIRECTIONS: Coord[] = [
  { x: 0, y: -1 }, // up
  { x: 1, y: 0 },  // right
  { x: 0, y: 1 },  // down
  { x: -1, y: 0 }, // left
];

export function coordToKey(coord: Coord): string {
  return `${coord.x},${coord.y}`;
}

export function keyToCoord(key: string): Coord {
  const [x, y] = key.split(',').map(Number);
  return { x, y };
}

export function coordsEqual(a: Coord, b: Coord): boolean {
  return a.x === b.x && a.y === b.y;
}

export function manhattanDistance(a: Coord, b: Coord): number {
  return Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
}

export function getNeighbors(coord: Coord, width: number, height: number): Coord[] {
  return DIRECTIONS
    .map(d => ({ x: coord.x + d.x, y: coord.y + d.y }))
    .filter(c => c.x >= 0 && c.x < width && c.y >= 0 && c.y < height);
}

export function isValidCoord(coord: Coord, width: number, height: number): boolean {
  return coord.x >= 0 && coord.x < width && coord.y >= 0 && coord.y < height;
}

// A* pathfinding with optional movement cost function
export function findPath(
  from: Coord,
  to: Coord,
  map: Tile[][],
  isPassable: (tile: Tile) => boolean,
  movementCost?: (tile: Tile) => number
): Coord[] | null {
  const width = map[0].length;
  const height = map.length;

  if (!isValidCoord(from, width, height) || !isValidCoord(to, width, height)) {
    return null;
  }

  const startTile = map[from.y][from.x];
  const endTile = map[to.y][to.x];

  if (!isPassable(startTile) || !isPassable(endTile)) {
    return null;
  }

  const openSet = new Set<string>([coordToKey(from)]);
  const cameFrom = new Map<string, string>();
  const gScore = new Map<string, number>();
  const fScore = new Map<string, number>();

  gScore.set(coordToKey(from), 0);
  fScore.set(coordToKey(from), manhattanDistance(from, to));

  while (openSet.size > 0) {
    // Find node with lowest fScore
    let currentKey = '';
    let lowestF = Infinity;
    for (const key of openSet) {
      const f = fScore.get(key) ?? Infinity;
      if (f < lowestF) {
        lowestF = f;
        currentKey = key;
      }
    }

    const current = keyToCoord(currentKey);

    if (coordsEqual(current, to)) {
      // Reconstruct path
      const path: Coord[] = [current];
      let key = currentKey;
      while (cameFrom.has(key)) {
        key = cameFrom.get(key)!;
        path.unshift(keyToCoord(key));
      }
      return path;
    }

    openSet.delete(currentKey);

    for (const neighbor of getNeighbors(current, width, height)) {
      const neighborTile = map[neighbor.y][neighbor.x];
      if (!isPassable(neighborTile)) continue;

      const neighborKey = coordToKey(neighbor);
      const tileCost = movementCost ? movementCost(neighborTile) : 1;
      const tentativeG = (gScore.get(currentKey) ?? Infinity) + tileCost;

      if (tentativeG < (gScore.get(neighborKey) ?? Infinity)) {
        cameFrom.set(neighborKey, currentKey);
        gScore.set(neighborKey, tentativeG);
        fScore.set(neighborKey, tentativeG + manhattanDistance(neighbor, to));
        openSet.add(neighborKey);
      }
    }
  }

  return null; // No path found
}

// Calculate the total movement cost of a path
export function getPathCost(
  path: Coord[],
  map: Tile[][],
  movementCost?: (tile: Tile) => number
): number {
  if (path.length < 2) return 0;

  let cost = 0;
  for (let i = 1; i < path.length; i++) {
    const tile = map[path[i].y][path[i].x];
    cost += movementCost ? movementCost(tile) : 1;
  }
  return cost;
}

// Get all tiles within movement range
// movementCost is optional - defaults to 1 for all tiles
export function getTilesInRange(
  from: Coord,
  range: number,
  map: Tile[][],
  isPassable: (tile: Tile) => boolean,
  movementCost?: (tile: Tile) => number
): Coord[] {
  const width = map[0].length;
  const height = map.length;
  const result: Coord[] = [];
  const costToReach = new Map<string, number>();
  const queue: { coord: Coord; cost: number }[] = [{ coord: from, cost: 0 }];

  costToReach.set(coordToKey(from), 0);

  while (queue.length > 0) {
    // Sort by cost to process lowest cost first (Dijkstra's algorithm)
    queue.sort((a, b) => a.cost - b.cost);
    const { coord, cost } = queue.shift()!;

    if (cost > 0) {
      result.push(coord);
    }

    if (cost < range) {
      for (const neighbor of getNeighbors(coord, width, height)) {
        const key = coordToKey(neighbor);
        const tile = map[neighbor.y][neighbor.x];
        if (!isPassable(tile)) continue;

        const tileCost = movementCost ? movementCost(tile) : 1;
        const newCost = cost + tileCost;

        // Only visit if we haven't found a cheaper path already
        if (newCost <= range && (!costToReach.has(key) || newCost < costToReach.get(key)!)) {
          costToReach.set(key, newCost);
          queue.push({ coord: neighbor, cost: newCost });
        }
      }
    }
  }

  return result;
}
