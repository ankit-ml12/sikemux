import { Seg } from "../../../plugin-api/ui";
import { formatAgo } from "../../../plugin-api/format";
import type { Container } from "../api";
import { isRunning, shortImage } from "../format";
import type { StateFilter } from "../state";
import { Ports, StateDot } from "./parts";

export function matches(container: Container, find: string, filter: StateFilter): boolean {
    if (filter === "running" && !isRunning(container)) return false;
    if (filter === "stopped" && isRunning(container)) return false;
    const words = find.trim().toLowerCase();
    if (!words) return true;
    return [container.name, container.image, container.compose?.project ?? "", container.compose?.service ?? ""].some((field) =>
        field.toLowerCase().includes(words),
    );
}

export function Containers({
    containers,
    find,
    filter,
    onFilter,
    onOpen,
}: {
    containers: Container[];
    find: string;
    filter: StateFilter;
    onFilter: (filter: StateFilter) => void;
    onOpen: (name: string) => void;
}) {
    const shown = containers.filter((container) => matches(container, find, filter));
    const found = containers.filter((container) => matches(container, find, "all"));
    const running = found.filter(isRunning).length;
    const filters = [
        { value: "all" as const, label: "All", count: found.length },
        { value: "running" as const, label: "Running", count: running },
        { value: "stopped" as const, label: "Stopped", count: found.length - running },
    ];
    return (
        <section className="docker-section" aria-label="Containers">
            <div className="docker-toolbar">
                <Seg label="Show" options={filters} value={filter} onChange={onFilter} />
            </div>
            {shown.length === 0 ? (
                <p className="docker-note">{containers.length === 0 ? "No containers on this engine yet." : "No containers match."}</p>
            ) : (
                <table className="docker-table" aria-label="Containers">
                    <thead>
                        <tr>
                            <th aria-label="State" />
                            <th>Name</th>
                            <th>Image</th>
                            <th>Status</th>
                            <th>Ports</th>
                            <th>Created</th>
                        </tr>
                    </thead>
                    <tbody>
                        {shown.map((container) => (
                            <tr
                                key={container.id}
                                className="docker-row"
                                tabIndex={0}
                                aria-label={container.name}
                                onClick={() => onOpen(container.name)}
                                onKeyDown={(event) => {
                                    if (event.key === "Enter") onOpen(container.name);
                                }}>
                                <td className="docker-cell-dot">
                                    <StateDot container={container} />
                                </td>
                                <td>
                                    <span className="docker-name">{container.name}</span>
                                    {container.compose ? (
                                        <span className="docker-chip" title={`Compose project ${container.compose.project}`}>
                                            {container.compose.project} · {container.compose.service}
                                        </span>
                                    ) : null}
                                </td>
                                <td className="docker-mono" title={container.image}>
                                    {shortImage(container.image)}
                                </td>
                                <td className="docker-muted">{container.status}</td>
                                <td>
                                    <Ports ports={container.ports} />
                                </td>
                                <td className="docker-muted">{formatAgo(container.created * 1000)}</td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            )}
        </section>
    );
}
