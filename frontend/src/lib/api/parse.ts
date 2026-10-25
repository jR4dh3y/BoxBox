/** Narrowing helpers for JSON responses. isRecord returns false; other helpers throw with the field label on a mismatch. */

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
