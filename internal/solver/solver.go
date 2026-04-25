package solver

import (
	"fmt"

	"hdd/internal/model"
)

type Action struct {
	Kind   string
	TileID int
}

type Decision struct {
	Action Action
	Done   bool
}

type ActionPlan struct {
	Actions   []Action
	Completed bool
}

func isSolvedSnapshot(snapshot model.SessionSnapshot) bool {
	return snapshot.Status == "won" || (len(snapshot.Tiles) == 0 && len(snapshot.SlotTiles) == 0)
}

func Next(snapshot model.SessionSnapshot) Decision {
	return NextWithForbidden(snapshot, nil)
}

func NextGreedy(snapshot model.SessionSnapshot) Decision {
	return NextGreedyWithForbidden(snapshot, nil)
}

func NextGreedyWithForbidden(snapshot model.SessionSnapshot, forbidden map[int]struct{}) Decision {
	if isSolvedSnapshot(snapshot) {
		return Decision{Done: true}
	}

	ordered := orderedTilesByIDDesc(cloneTiles(snapshot.Tiles), cloneForbidden(forbidden))
	if len(ordered) == 0 {
		return fallback(snapshot)
	}

	return Decision{Action: Action{Kind: "click", TileID: ordered[0].ID}}
}

func NextWithForbidden(snapshot model.SessionSnapshot, forbidden map[int]struct{}) Decision {
	if isSolvedSnapshot(snapshot) {
		return Decision{Done: true}
	}

	if tileID, ok := FindWinningClickWithForbidden(snapshot, forbidden); ok {
		return Decision{Action: Action{Kind: "click", TileID: tileID}}
	}

	return NextGreedyWithForbidden(snapshot, forbidden)
}

func PlanToToolBoundary(snapshot model.SessionSnapshot) (ActionPlan, error) {
	return PlanToToolBoundaryWithBudget(snapshot, 2000, 2)
}

func PlanToToolBoundaryWithBudget(snapshot model.SessionSnapshot, initialBudget int, attempts int) (ActionPlan, error) {
	budget := initialBudget
	if budget <= 0 {
		budget = 2000
	}
	if attempts <= 0 {
		attempts = 2
	}
	for attempt := 0; attempt < attempts; attempt++ {
		plan, solved, cutoff, err := planClickOnly(snapshot, budget)
		if err != nil {
			return ActionPlan{}, err
		}
		if solved {
			return plan, nil
		}
		if !cutoff {
			boundaryPlan, _, _, err := planToPowerupBoundary(snapshot, budget)
			if err != nil {
				return ActionPlan{}, err
			}
			if len(boundaryPlan.Actions) > 0 {
				return boundaryPlan, nil
			}
			break
		}
		budget *= 2
	}
	decision := Next(snapshot)
	if decision.Done {
		return ActionPlan{}, fmt.Errorf("当前整局无法生成可执行计划")
	}
	return ActionPlan{Actions: []Action{decision.Action}, Completed: false}, nil
}

func fallback(snapshot model.SessionSnapshot) Decision {
	if snapshot.Powerups.Undo > 0 && len(snapshot.SlotTiles) > 0 {
		return Decision{Action: Action{Kind: "undo"}}
	}
	if snapshot.Powerups.Remove > 0 && len(snapshot.SlotTiles) >= 3 {
		return Decision{Action: Action{Kind: "remove"}}
	}
	if snapshot.Powerups.Shuffle > 0 {
		return Decision{Action: Action{Kind: "shuffle"}}
	}
	return Decision{Done: true}
}
