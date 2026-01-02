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

// A* pathfinding
export function findPath(
  from: Coord,
  to: Coord,
  map: Tile[][],
  isPassable: (tile: Tile) => boolean
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
      const tentativeG = (gScore.get(currentKey) ?? Infinity) + 1;

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

// Get all tiles within movement range
export function getTilesInRange(
  from: Coord,
  range: number,
  map: Tile[][],
  isPassable: (tile: Tile) => boolean
): Coord[] {
  const width = map[0].length;
  const height = map.length;
  const result: Coord[] = [];
  const visited = new Set<string>();
  const queue: { coord: Coord; distance: number }[] = [{ coord: from, distance: 0 }];

  visited.add(coordToKey(from));

  while (queue.length > 0) {
    const { coord, distance } = queue.shift()!;

    if (distance > 0) {
      result.push(coord);
    }

    if (distance < range) {
      for (const neighbor of getNeighbors(coord, width, height)) {
        const key = coordToKey(neighbor);
        if (visited.has(key)) continue;

        const tile = map[neighbor.y][neighbor.x];
        if (!isPassable(tile)) continue;

        visited.add(key);
        queue.push({ coord: neighbor, distance: distance + 1 });
      }
    }
  }

  return result;
}
