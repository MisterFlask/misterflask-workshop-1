// node --test: the tech tree's layout (app/empire.js `treeLayout`): columns by prerequisite
// depth, rows ordered so that a tech sits near its prerequisites.
import test from "node:test";
import assert from "node:assert/strict";
import { treeLayout } from "../app/empire.js";

const tech = (id, depth, and = [], or = [], era = 0) => ({ tech: id, depth, and, or, era });

test("columns by depth; rows follow the prerequisites", () => {
  const { row, columns, rows } = treeLayout([
    tech(0, 0),
    tech(1, 0),
    tech(2, 1, [1]),
    tech(3, 1, [0]),
    tech(4, 2, [], [2, 3]),
    tech(5, 0, [], [], 1),
  ]);
  assert.equal(columns, 3);
  assert.equal(rows, 3);
  // Depth 0 by era then index; depth 1: 3 (after 0) above 2 (after 1).
  assert.deepEqual([row.get(0), row.get(1), row.get(5)], [0, 1, 2]);
  assert.deepEqual([row.get(3), row.get(2)], [0, 1]);
  assert.equal(row.get(4), 0);
  assert.deepEqual(treeLayout([]), { row: new Map(), columns: 0, rows: 0 });
});
