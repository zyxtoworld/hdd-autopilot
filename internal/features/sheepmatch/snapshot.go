package sheepmatch

import (
	"fmt"
	"sort"
	"strings"

	"hdd/internal/model"
	"hdd/internal/solver"
)

func snapshotFromStartResponse(start *model.StartResponse) model.SessionSnapshot {
	if start == nil {
		return model.SessionSnapshot{}
	}
	return model.SessionSnapshot{
		Difficulty: start.Difficulty,
		SessionID:  start.SessionID,
		SlotLimit:  start.SlotLimit,
		Powerups:   start.Powerups,
		Status:     start.Status,
		Tiles:      cloneTilesWithoutIDs(start.Tiles, start.Slots),
		SlotTiles:  cloneSlotTiles(start.SlotTiles, start.Slots, start.Tiles),
		MoveCount:  start.MoveCount,
	}
}

func snapshotFromHistoryItem(item *model.HistoryItem) model.SessionSnapshot {
	if item == nil {
		return model.SessionSnapshot{}
	}
	return model.SessionSnapshot{
		Difficulty: item.Difficulty,
		SessionID:  item.SessionID,
		SlotLimit:  item.SlotLimit,
		Powerups:   item.Powerups,
		Status:     item.Status,
		Tiles:      cloneTilesWithoutIDs(item.Tiles, item.Slots),
		SlotTiles:  cloneSlotTiles(item.SlotTiles, item.Slots, item.Tiles),
		MoveCount:  item.MoveCount,
	}
}

func historyItemToStartResponse(item *model.HistoryItem) *model.StartResponse {
	if item == nil {
		return nil
	}
	return &model.StartResponse{
		Difficulty: item.Difficulty,
		MoveCount:  item.MoveCount,
		Powerups:   item.Powerups,
		SessionID:  item.SessionID,
		SlotLimit:  item.SlotLimit,
		Slots:      append([]int(nil), item.Slots...),
		SlotTiles:  cloneTiles(item.SlotTiles),
		Status:     item.Status,
		Tiles:      cloneTiles(item.Tiles),
	}
}

func orderedTilesByIDDesc(tiles []model.Tile) []model.Tile {
	ordered := cloneTiles(tiles)
	sort.Slice(ordered, func(i, j int) bool {
		return ordered[i].ID > ordered[j].ID
	})
	return ordered
}

func applyPlannedActionLocally(snapshot model.SessionSnapshot, action solver.Action) (model.SessionSnapshot, error) {
	switch action.Kind {
	case "click":
		return applyLocalClickSnapshot(snapshot, action.TileID)
	case "undo", "remove", "shuffle":
		next := snapshot
		next.Status = action.Kind
		return next, nil
	case "abandon":
		next := snapshot
		next.Status = "abandoned"
		return next, nil
	default:
		return model.SessionSnapshot{}, fmt.Errorf("不支持的动作：%s", action.Kind)
	}
}

func applyLocalClickSnapshot(snapshot model.SessionSnapshot, tileID int) (model.SessionSnapshot, error) {
	ordered := orderedTilesByIDDesc(snapshot.Tiles)
	for _, tile := range ordered {
		if tile.ID != tileID {
			continue
		}
		next := snapshot
		next.Tiles = removeTileByID(snapshot.Tiles, tileID)
		next.SlotTiles = append(cloneTiles(snapshot.SlotTiles), tile)
		if len(next.SlotTiles) > next.SlotLimit {
			next.Status = "failed"
			return next, nil
		}
		patternCount := 0
		for _, slotTile := range next.SlotTiles {
			if slotTile.Pattern == tile.Pattern {
				patternCount++
			}
		}
		if patternCount >= 3 {
			filtered := make([]model.Tile, 0, len(next.SlotTiles)-3)
			removed := 0
			for _, slotTile := range next.SlotTiles {
				if slotTile.Pattern == tile.Pattern && removed < 3 {
					removed++
					continue
				}
				filtered = append(filtered, slotTile)
			}
			next.SlotTiles = filtered
		}
		next.MoveCount++
		if len(next.Tiles) == 0 && len(next.SlotTiles) == 0 {
			next.Status = "won"
		} else {
			next.Status = snapshot.Status
		}
		return next, nil
	}
	return model.SessionSnapshot{}, fmt.Errorf("目标方块已不在棋盘上")
}

func isSlotFullError(err error) bool {
	if err == nil {
		return false
	}
	message := strings.ToLower(strings.TrimSpace(err.Error()))
	return strings.Contains(message, "槽位已满") || strings.Contains(message, "slot full")
}

func snapshotFromStepResponse(previous model.SessionSnapshot, stepResp *model.StepResponse) model.SessionSnapshot {
	if stepResp == nil {
		return previous
	}
	next := previous
	next.MoveCount = stepResp.MoveCount
	next.Status = stepResp.Status
	if stepResp.SessionID != 0 {
		next.SessionID = stepResp.SessionID
	}
	if stepResp.SlotLimit > 0 {
		next.SlotLimit = stepResp.SlotLimit
	}
	next.Powerups = stepResp.Powerups
	next.SlotTiles = resolveSlotTiles(previous, stepResp)
	next.Tiles = cloneTilesWithoutIDs(stepResp.Tiles, collectTileIDs(next.SlotTiles))
	if len(stepResp.Removed) > 0 {
		next.Tiles = removeTilesByID(next.Tiles, stepResp.Removed)
	}
	return next
}

func resolveSlotTiles(previous model.SessionSnapshot, stepResp *model.StepResponse) []model.Tile {
	if len(stepResp.Slots) == 0 {
		return nil
	}
	lookup := make(map[int]model.Tile, len(previous.SlotTiles)+len(previous.Tiles)+len(stepResp.Tiles))
	for _, tile := range previous.SlotTiles {
		lookup[tile.ID] = tile
	}
	for _, tile := range previous.Tiles {
		lookup[tile.ID] = tile
	}
	for _, tile := range stepResp.Tiles {
		lookup[tile.ID] = tile
	}
	result := make([]model.Tile, 0, len(stepResp.Slots))
	for _, id := range stepResp.Slots {
		if tile, ok := lookup[id]; ok {
			result = append(result, tile)
		}
	}
	return result
}

func collectTileIDs(tiles []model.Tile) []int {
	ids := make([]int, 0, len(tiles))
	for _, tile := range tiles {
		ids = append(ids, tile.ID)
	}
	return ids
}

func cloneTilesWithoutIDs(tiles []model.Tile, excluded []int) []model.Tile {
	excludedSet := make(map[int]struct{}, len(excluded))
	for _, id := range excluded {
		excludedSet[id] = struct{}{}
	}
	result := make([]model.Tile, 0, len(tiles))
	for _, tile := range tiles {
		if _, ok := excludedSet[tile.ID]; ok {
			continue
		}
		result = append(result, tile)
	}
	return result
}

func cloneSlotTiles(explicit []model.Tile, slots []int, board []model.Tile) []model.Tile {
	if len(explicit) > 0 {
		return cloneTiles(explicit)
	}
	if len(slots) == 0 {
		return nil
	}
	lookup := make(map[int]model.Tile, len(board))
	for _, tile := range board {
		lookup[tile.ID] = tile
	}
	result := make([]model.Tile, 0, len(slots))
	for _, id := range slots {
		if tile, ok := lookup[id]; ok {
			result = append(result, tile)
		}
	}
	return result
}

func removeTileByID(tiles []model.Tile, tileID int) []model.Tile {
	result := make([]model.Tile, 0, len(tiles))
	for _, tile := range tiles {
		if tile.ID == tileID {
			continue
		}
		result = append(result, tile)
	}
	return result
}

func removeTilesByID(tiles []model.Tile, ids []int) []model.Tile {
	if len(ids) == 0 {
		return cloneTiles(tiles)
	}
	removed := make(map[int]struct{}, len(ids))
	for _, id := range ids {
		removed[id] = struct{}{}
	}
	result := make([]model.Tile, 0, len(tiles))
	for _, tile := range tiles {
		if _, ok := removed[tile.ID]; ok {
			continue
		}
		result = append(result, tile)
	}
	return result
}

func cloneTiles(tiles []model.Tile) []model.Tile {
	result := make([]model.Tile, len(tiles))
	copy(result, tiles)
	return result
}
