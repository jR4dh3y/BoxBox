/** Narrowing helpers for JSON responses. Each throws with the field label on a mismatch. */

export function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function record(value: unknown, label: string): Record<string, unknown> {
	if (!isRecord(value)) throw new Error(`Invalid ${label}`);
	return value;
}

export function array(value: unknown, label: string): unknown[] {
	if (!Array.isArray(value)) throw new Error(`Invalid ${label}`);
	return value;
}

export function string(value: unknown, label: string): string {
	if (typeof value !== 'string') throw new Error(`Invalid ${label}`);
	return value;
}

export function nonEmptyString(value: unknown, label: string): string {
	const text = string(value, label);
	if (text.length === 0) throw new Error(`Invalid ${label}`);
	return text;
}

export function optionalString(value: unknown, label: string): string | undefined {
	return value === undefined || value === null ? undefined : string(value, label);
}

export function boolean(value: unknown, label: string): boolean {
	if (typeof value !== 'boolean') throw new Error(`Invalid ${label}`);
	return value;
}

export function optionalByteCount(value: unknown, label: string): number | undefined {
	if (value === undefined || value === null) return undefined;
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
		throw new Error(`Invalid ${label}`);
	}
	return value;
}

/** Desktop mutations answer `{ ok: true }`. */
export function ok(value: unknown, label: string): void {
	if (record(value, label).ok !== true) throw new Error(`Invalid ${label}`);
}
