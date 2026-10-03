export interface QualityAuditPosition {
  pageIndex: number;
  unitId: string | null;
}

export interface QualityAuditNavigationTarget {
  unitId: string;
  unitIndex: number;
  pageIndex: number;
}

function normalizedPageSize(pageSize: number): number {
  return Number.isFinite(pageSize) && pageSize > 0 ? Math.floor(pageSize) : 1;
}

export function pageIndexForUnit(unitIndex: number, pageSize: number): number {
  return Math.floor(Math.max(0, unitIndex) / normalizedPageSize(pageSize));
}

export function restoreQualityAuditPosition(
  unitIds: readonly string[],
  pageSize: number,
  position?: QualityAuditPosition,
): QualityAuditPosition {
  if (!unitIds.length) return { pageIndex: 0, unitId: null };
  const selectedIndex = position?.unitId ? unitIds.indexOf(position.unitId) : -1;
  if (selectedIndex >= 0) {
    return {
      pageIndex: pageIndexForUnit(selectedIndex, pageSize),
      unitId: unitIds[selectedIndex],
    };
  }
  const maximumPage = pageIndexForUnit(unitIds.length - 1, pageSize);
  const requestedPage = Math.min(Math.max(0, position?.pageIndex ?? 0), maximumPage);
  const unitIndex = Math.min(requestedPage * normalizedPageSize(pageSize), unitIds.length - 1);
  return { pageIndex: requestedPage, unitId: unitIds[unitIndex] };
}

export function moveQualityAuditUnit(
  unitIds: readonly string[],
  pageSize: number,
  currentUnitId: string | null,
  currentPageIndex: number,
  offset: -1 | 1,
): QualityAuditNavigationTarget | null {
  if (!unitIds.length) return null;
  const selectedIndex = currentUnitId ? unitIds.indexOf(currentUnitId) : -1;
  const fallbackIndex = Math.min(
    Math.max(0, currentPageIndex * normalizedPageSize(pageSize)),
    unitIds.length - 1,
  );
  const currentIndex = selectedIndex >= 0 ? selectedIndex : fallbackIndex;
  const targetIndex = currentIndex + offset;
  if (targetIndex < 0 || targetIndex >= unitIds.length) return null;
  return {
    unitId: unitIds[targetIndex],
    unitIndex: targetIndex,
    pageIndex: pageIndexForUnit(targetIndex, pageSize),
  };
}
