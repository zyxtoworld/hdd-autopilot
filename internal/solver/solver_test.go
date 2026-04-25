package solver

import (
	"testing"

	"hdd/internal/model"
)

func TestNextWithForbiddenSkipsRejectedTile(t *testing.T) {
	snapshot := model.SessionSnapshot{
		SlotLimit: 7,
		Tiles: []model.Tile{
			{ID: 1, GX: 0, GY: 0, Layer: 0, Pattern: "P1"},
			{ID: 2, GX: 2, GY: 0, Layer: 0, Pattern: "P1"},
			{ID: 3, GX: 4, GY: 0, Layer: 0, Pattern: "P1"},
		},
	}

	decision := NextWithForbidden(snapshot, map[int]struct{}{1: {}})
	if decision.Done {
		t.Fatalf("expected a fallback click")
	}
	if decision.Action.Kind != "click" {
		t.Fatalf("expected click action, got %s", decision.Action.Kind)
	}
	if decision.Action.TileID == 1 {
		t.Fatalf("expected solver to skip forbidden tile 1")
	}
}

func TestNextGreedyUsesHighestIDOnFreshBoard(t *testing.T) {
	snapshot := model.SessionSnapshot{
		SlotLimit: 7,
		Tiles: []model.Tile{
			{ID: 46, GX: 1, GY: 1, Layer: 3, Pattern: "P6"},
			{ID: 48, GX: 0, GY: 5, Layer: 3, Pattern: "P6"},
			{ID: 47, GX: 4, GY: 2, Layer: 3, Pattern: "P6"},
			{ID: 45, GX: 3, GY: 3, Layer: 3, Pattern: "P3"},
			{ID: 43, GX: 3, GY: 5, Layer: 3, Pattern: "P3"},
			{ID: 44, GX: 2, GY: 3, Layer: 3, Pattern: "P3"},
		},
	}

	first := NextGreedy(snapshot)
	if first.Done || first.Action.Kind != "click" {
		t.Fatalf("expected first greedy action to be a click, got %+v", first)
	}
	if first.Action.TileID != 48 {
		t.Fatalf("expected first greedy click to choose highest id 48, got %d", first.Action.TileID)
	}
}

func TestNextGreedyUsesHighestIDOnContinuedBoard(t *testing.T) {
	snapshot := model.SessionSnapshot{
		SlotLimit: 7,
		SlotTiles: []model.Tile{
			{ID: 11, GX: 0, GY: 0, Layer: 0, Pattern: "A"},
		},
		Tiles: []model.Tile{
			{ID: 41, GX: 1, GY: 0, Layer: 1, Pattern: "B"},
			{ID: 55, GX: 2, GY: 0, Layer: 3, Pattern: "C"},
			{ID: 49, GX: 3, GY: 0, Layer: 2, Pattern: "A"},
		},
	}

	first := NextGreedy(snapshot)
	if first.Done || first.Action.Kind != "click" {
		t.Fatalf("expected greedy action to be a click, got %+v", first)
	}
	if first.Action.TileID != 55 {
		t.Fatalf("expected continued board to choose highest id 55, got %d", first.Action.TileID)
	}
}

func TestFindWinningPathReturnsDescendingIDPath(t *testing.T) {
	snapshot := model.SessionSnapshot{
		SlotLimit: 7,
		Tiles: []model.Tile{
			{ID: 1, GX: 0, GY: 0, Layer: 0, Pattern: "P1"},
			{ID: 2, GX: 2, GY: 0, Layer: 0, Pattern: "P1"},
			{ID: 3, GX: 4, GY: 0, Layer: 0, Pattern: "P1"},
		},
	}

	path, ok, cutoff := FindWinningPath(snapshot, searchBudget)
	if cutoff {
		t.Fatalf("expected full search without cutoff")
	}
	if !ok {
		t.Fatalf("expected winning path")
	}
	if len(path) != 3 {
		t.Fatalf("expected full path of 3 clicks, got %d", len(path))
	}
	if path[0] != 3 || path[1] != 2 || path[2] != 1 {
		t.Fatalf("expected descending id path [3 2 1], got %v", path)
	}
}

func TestFindWinningPathReportsCutoffWhenBudgetTooSmall(t *testing.T) {
	snapshot := model.SessionSnapshot{
		SlotLimit: 7,
		Tiles: []model.Tile{
			{ID: 1, GX: 0, GY: 0, Layer: 0, Pattern: "P1"},
			{ID: 2, GX: 2, GY: 0, Layer: 0, Pattern: "P1"},
			{ID: 3, GX: 4, GY: 0, Layer: 0, Pattern: "P1"},
		},
	}

	_, ok, cutoff := FindWinningPath(snapshot, 1)
	if ok {
		t.Fatalf("did not expect solve with tiny budget")
	}
	if !cutoff {
		t.Fatalf("expected cutoff when budget is exhausted")
	}
}

func TestPlanToToolBoundaryReturnsFullWinningPlan(t *testing.T) {
	snapshot := model.SessionSnapshot{
		SlotLimit: 7,
		Tiles: []model.Tile{
			{ID: 1, GX: 0, GY: 0, Layer: 0, Pattern: "A"},
			{ID: 2, GX: 2, GY: 0, Layer: 0, Pattern: "A"},
			{ID: 3, GX: 4, GY: 0, Layer: 0, Pattern: "A"},
		},
	}

	plan, err := PlanToToolBoundary(snapshot)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !plan.Completed {
		t.Fatalf("expected completed click-only plan")
	}
	if len(plan.Actions) != 3 {
		t.Fatalf("expected 3 planned clicks, got %d", len(plan.Actions))
	}
	for _, action := range plan.Actions {
		if action.Kind != "click" {
			t.Fatalf("expected only click actions, got %s", action.Kind)
		}
	}
	if plan.Actions[0].TileID != 3 || plan.Actions[1].TileID != 2 || plan.Actions[2].TileID != 1 {
		t.Fatalf("expected descending id plan [3 2 1], got [%d %d %d]", plan.Actions[0].TileID, plan.Actions[1].TileID, plan.Actions[2].TileID)
	}
}

func TestPlanToToolBoundaryKeepsDeepeningBeforePowerup(t *testing.T) {
	snapshot := model.SessionSnapshot{
		SlotLimit: 7,
		Powerups:  model.Powerups{Undo: 1},
		Tiles: []model.Tile{
			{ID: 1, GX: 0, GY: 0, Layer: 0, Pattern: "A"},
			{ID: 2, GX: 2, GY: 0, Layer: 0, Pattern: "A"},
			{ID: 3, GX: 4, GY: 0, Layer: 0, Pattern: "A"},
		},
	}

	plan, err := PlanToToolBoundaryWithBudget(snapshot, 1, 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(plan.Actions) == 0 {
		t.Fatalf("expected bounded planner to return playable actions")
	}
	if plan.Actions[0].Kind != "click" {
		t.Fatalf("expected planner to prefer click before powerups, got %s", plan.Actions[0].Kind)
	}
}

func TestPlanToToolBoundaryUsesPowerupOnlyAfterSearchFullyExplored(t *testing.T) {
	snapshot := model.SessionSnapshot{
		SlotLimit: 1,
		Powerups:  model.Powerups{Undo: 1},
		Tiles: []model.Tile{
			{ID: 1, GX: 0, GY: 0, Layer: 0, Pattern: "A"},
			{ID: 2, GX: 2, GY: 0, Layer: 0, Pattern: "B"},
		},
	}

	plan, err := PlanToToolBoundaryWithBudget(snapshot, searchBudget, 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(plan.Actions) != 2 {
		t.Fatalf("expected click plus powerup boundary, got %d actions", len(plan.Actions))
	}
	if plan.Actions[0].Kind != "click" {
		t.Fatalf("expected first action click, got %s", plan.Actions[0].Kind)
	}
	if plan.Actions[1].Kind != "undo" {
		t.Fatalf("expected second action undo, got %s", plan.Actions[1].Kind)
	}
	if plan.Completed {
		t.Fatalf("expected plan to stop at tool boundary")
	}
}

func TestPlanToToolBoundaryWithBudgetReturnsAfterCutoff(t *testing.T) {
	snapshot := model.SessionSnapshot{
		SlotLimit: 1,
		Tiles: []model.Tile{
			{ID: 1, GX: 0, GY: 0, Layer: 0, Pattern: "A"},
			{ID: 2, GX: 2, GY: 0, Layer: 0, Pattern: "B"},
			{ID: 3, GX: 4, GY: 0, Layer: 0, Pattern: "C"},
			{ID: 4, GX: 6, GY: 0, Layer: 0, Pattern: "D"},
		},
	}

	plan, err := PlanToToolBoundaryWithBudget(snapshot, 1, 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(plan.Actions) == 0 {
		t.Fatalf("expected bounded planner to return at least one action after cutoff")
	}
	if plan.Actions[0].Kind == "abandon" {
		t.Fatalf("expected playable fallback action, got abandon")
	}
}
