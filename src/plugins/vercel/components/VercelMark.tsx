/** Vercel's mark, the triangle. */
export function VercelMark({ size = 16 }: { size?: number }) {
    return (
        <svg width={size} height={size} viewBox="0 0 76 76" aria-hidden="true">
            <path fill="currentColor" d="M38 8 74 70H2Z" />
        </svg>
    );
}
