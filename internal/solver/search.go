package solver

import (
	"sort"
	"strconv"
	"strings"

	"hdd/internal/model"
)

const searchBudget = 50000

type searchState struct {
	board     []model.Tile
	slots     []model.Tile
	slotLimit int
	forbidden map[int]struct{}
}

type searchContext struct {
	visited   int
	budget    int
	budgetHit bool
	failed    map[string]struct{}
}

type searchResult struct {
	actions []int
	solved  bool
	cutoff  bool
}

type boundarySearchResult struct {
	actions  []Action
	solved   bool
	boundary bool
	cutoff   bool
}

func FindWinningClickWithForbidden(snapshot model.SessionSnapshot, forbidden map[int]struct{}) (int, bool) {
	actions, ok, _ := FindWinningPathWithForbidden(snapshot, searchBudget, forbidden)
	if !ok || len(actions) == 0 {
		return 0, false
	}
	return actions[0], true
}

func FindWinningPath(snapshot model.SessionSnapshot, budget int) ([]int, bool, bool) {
	return FindWinningPathWithForbidden(snapshot, budget, nil)
}

func FindWinningPathWithForbidden(snapshot model.SessionSnapshot, budget int, forbidden map[int]struct{}) ([]int, bool, bool) {
	ctx := &searchContext{
		budget: budget,
		failed: map[string]struct{}{},
	}
	state := searchState{
		board:     cloneTiles(snapshot.Tiles),
		slots:     cloneTiles(snapshot.SlotTiles),
		slotLimit: snapshot.SlotLimit,
		forbidden: cloneForbidden(forbidden),
	}
	result := findWinningPath(ctx, state)
	return result.actions, result.solved, result.cutoff || ctx.budgetHit
}

func findWinningPath(ctx *searchContext, state searchState) searchResult {
	if len(state.board) == 0 {
		return searchResult{solved: len(state.slots) == 0}
	}
	if ctx.visited >= ctx.budget {
		ctx.budgetHit = true
		return searchResult{cutoff: true}
	}
	ctx.visited++

	key := stateKey(state)
	if _, ok := ctx.failed[key]; ok {
		return searchResult{}
	}

	tiles := orderedTilesByIDDesc(state.board, state.forbidden)
	cutoff := false
	for _, tile := range tiles {
		next, ok := applyClick(state, tile.ID)
		if !ok {
			continue
		}
		result := findWinningPath(ctx, next)
		if result.solved {
			actions := make([]int, 0, len(result.actions)+1)
			actions = append(actions, tile.ID)
			actions = append(actions, result.actions...)
			return searchResult{actions: actions, solved: true}
		}
		if result.cutoff {
			cutoff = true
		}
	}

	if cutoff {
		return searchResult{cutoff: true}
	}
	ctx.failed[key] = struct{}{}
	return searchResult{}
}

func planToPowerupBoundary(snapshot model.SessionSnapshot, budget int) (ActionPlan, bool, bool, error) {
	ctx := &searchContext{
		budget: budget,
		failed: map[string]struct{}{},
	}
	state := searchState{
		board:     cloneTiles(snapshot.Tiles),
		slots:     cloneTiles(snapshot.SlotTiles),
		slotLimit: snapshot.SlotLimit,
		forbidden: nil,
	}
	result := findPlanToPowerupBoundary(ctx, snapshot.Powerups, state)
	if result.cutoff || ctx.budgetHit {
		return ActionPlan{}, false, true, nil
	}
	if result.solved || result.boundary {
		return ActionPlan{Actions: result.actions, Completed: result.solved}, true, false, nil
	}
	return ActionPlan{}, false, false, nil
}

func findPlanToPowerupBoundary(ctx *searchContext, powerups model.Powerups, state searchState) boundarySearchResult {
	if len(state.board) == 0 {
		if len(state.slots) == 0 {
			return boundarySearchResult{solved: true}
		}
		if action, ok := availablePowerupBoundary(powerups, state.slots); ok {
			return boundarySearchResult{actions: []Action{action}, boundary: true}
		}
		return boundarySearchResult{}
	}
	if ctx.visited >= ctx.budget {
		ctx.budgetHit = true
		return boundarySearchResult{cutoff: true}
	}
	ctx.visited++

	key := stateKey(state)
	if _, ok := ctx.failed[key]; ok {
		return boundarySearchResult{}
	}

	tiles := orderedTilesByIDDesc(state.board, state.forbidden)
	cutoff := false
	var boundaryPlan []Action
	for _, tile := range tiles {
		next, ok := applyClick(state, tile.ID)
		if !ok {
			continue
		}
		result := findPlanToPowerupBoundary(ctx, powerups, next)
		if result.solved {
			actions := make([]Action, 0, len(result.actions)+1)
			actions = append(actions, Action{Kind: "click", TileID: tile.ID})
			actions = append(actions, result.actions...)
			return boundarySearchResult{actions: actions, solved: true}
		}
		if result.cutoff {
			cutoff = true
			continue
		}
		if result.boundary && len(boundaryPlan) == 0 {
			actions := make([]Action, 0, len(result.actions)+1)
			actions = append(actions, Action{Kind: "click", TileID: tile.ID})
			actions = append(actions, result.actions...)
			boundaryPlan = actions
		}
	}

	if cutoff {
		return boundarySearchResult{cutoff: true}
	}
	if len(boundaryPlan) > 0 {
		return boundarySearchResult{actions: boundaryPlan, boundary: true}
	}
	if action, ok := availablePowerupBoundary(powerups, state.slots); ok {
		return boundarySearchResult{actions: []Action{action}, boundary: true}
	}
	ctx.failed[key] = struct{}{}
	return boundarySearchResult{}
}

func availablePowerupBoundary(powerups model.Powerups, slots []model.Tile) (Action, bool) {
	if powerups.Undo > 0 && len(slots) > 0 {
		return Action{Kind: "undo"}, true
	}
	if powerups.Remove > 0 && len(slots) >= 3 {
		return Action{Kind: "remove"}, true
	}
	if powerups.Shuffle > 0 {
		return Action{Kind: "shuffle"}, true
	}
	return Action{}, false
}

func orderedTilesByIDDesc(tiles []model.Tile, forbidden map[int]struct{}) []model.Tile {
	ordered := make([]model.Tile, 0, len(tiles))
	for _, tile := range tiles {
		if _, blocked := forbidden[tile.ID]; blocked {
			continue
		}
		ordered = append(ordered, tile)
	}
	sort.SliceStable(ordered, func(i, j int) bool {
		return ordered[i].ID > ordered[j].ID
	})
	return ordered
}

func applyClick(state searchState, tileID int) (searchState, bool) {
	board := make([]model.Tile, 0, len(state.board)-1)
	var clicked model.Tile
	found := false
	for _, tile := range state.board {
		if tile.ID == tileID {
			clicked = tile
			found = true
			continue
		}
		board = append(board, tile)
	}
	if !found {
		return searchState{}, false
	}
	if _, blocked := state.forbidden[tileID]; blocked {
		return searchState{}, false
	}

	slots := append(cloneTiles(state.slots), clicked)
	patternCount := 0
	for _, tile := range slots {
		if tile.Pattern == clicked.Pattern {
			patternCount++
		}
	}
	if patternCount >= 3 {
		filtered := make([]model.Tile, 0, len(slots)-3)
		removed := 0
		for _, tile := range slots {
			if tile.Pattern == clicked.Pattern && removed < 3 {
				removed++
				continue
			}
			filtered = append(filtered, tile)
		}
		slots = filtered
	}
	if len(slots) > state.slotLimit {
		return searchState{}, false
	}
	return searchState{board: board, slots: slots, slotLimit: state.slotLimit, forbidden: cloneForbidden(state.forbidden)}, true
}

func stateKey(state searchState) string {
	boardIDs := make([]int, 0, len(state.board))
	for _, tile := range state.board {
		boardIDs = append(boardIDs, tile.ID)
	}
	sort.Ints(boardIDs)

	slotIDs := make([]int, 0, len(state.slots))
	for _, tile := range state.slots {
		slotIDs = append(slotIDs, tile.ID)
	}
	sort.Ints(slotIDs)

	forbiddenIDs := make([]int, 0, len(state.forbidden))
	for id := range state.forbidden {
		forbiddenIDs = append(forbiddenIDs, id)
	}
	sort.Ints(forbiddenIDs)

	var builder strings.Builder
	for _, id := range boardIDs {
		builder.WriteString(strconv.Itoa(id))
		builder.WriteByte(',')
	}
	builder.WriteByte('|')
	for _, id := range slotIDs {
		builder.WriteString(strconv.Itoa(id))
		builder.WriteByte(',')
	}
	builder.WriteByte('|')
	for _, id := range forbiddenIDs {
		builder.WriteString(strconv.Itoa(id))
		builder.WriteByte(',')
	}
	return builder.String()
}

func cloneTiles(tiles []model.Tile) []model.Tile {
	result := make([]model.Tile, len(tiles))
	copy(result, tiles)
	return result
}

func cloneForbidden(values map[int]struct{}) map[int]struct{} {
	if len(values) == 0 {
		return nil
	}
	result := make(map[int]struct{}, len(values))
	for id := range values {
		result[id] = struct{}{}
	}
	return result
}

func planClickOnly(snapshot model.SessionSnapshot, budget int) (ActionPlan, bool, bool, error) {
	path, ok, cutoff := FindWinningPath(snapshot, budget)
	if ok {
		actions := make([]Action, 0, len(path))
		for _, tileID := range path {
			actions = append(actions, Action{Kind: "click", TileID: tileID})
		}
		return ActionPlan{Actions: actions, Completed: true}, true, false, nil
	}
	return ActionPlan{}, false, cutoff, nil
}
