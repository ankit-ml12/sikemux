import { useResourceEnabled } from "../../../plugin-api/resources";
import { runInTerminal, swallow } from "../../../plugin-api/host";
import { gcpApi, type Target, type VmDetail, type VmInstance } from "../api";
import { instanceR, instancesR, metricsR, serialR } from "../resources";
import { splitOpen, updateGcpView, useGcpView } from "../state";
import type { ProductProps } from "./GcpPane";
import { LogStream } from "./LogStream";
import { SendToAgent } from "./SendToAgent";
import { serialForAgent } from "../forAgent";
import { Filter } from "./RunPage";
import { ago, Button, consoleUrl, Copyable, Facts, keyOf, Loaded, PageHeader, size, Sparkline, Status, Table, Tabs, useAction, visit } from "./parts";

type Power = "start" | "stop" | "reset" | "suspend" | "resume";

const POWER: Record<Power, { label: string; body: (name: string) => string; destructive?: boolean }> = {
    start: { label: "Start", body: (name) => `${name} boots, and is billed while it runs.` },
    stop: { label: "Stop", body: (name) => `${name} shuts down. Its disks are kept, and anything in memory is lost.`, destructive: true },
    reset: { label: "Reset", body: (name) => `${name} restarts at once, as a power cycle does, without shutting down cleanly.`, destructive: true },
    suspend: { label: "Suspend", body: (name) => `${name}'s memory is saved to disk and it stops running until resumed.` },
    resume: { label: "Resume", body: (name) => `${name} carries on from where it was suspended.` },
};

/** Which power changes make sense for a VM in its current state. */
export function powersFor(status: string): Power[] {
    switch (status) {
        case "RUNNING":
            return ["stop", "reset", "suspend"];
        case "TERMINATED":
            return ["start"];
        case "SUSPENDED":
            return ["resume"];
        default:
            return [];
    }
}

export function ComputePage({ paneId, active, target }: ProductProps) {
    const view = useGcpView(paneId);
    const opened = splitOpen(view.open);
    if (opened)
        return (
            <InstancePage
                paneId={paneId}
                active={active}
                target={target}
                zone={opened.location}
                name={opened.name}
                onBack={() => updateGcpView(paneId, { open: null, tab: null })}
            />
        );
    return <InstanceList paneId={paneId} active={active} target={target} />;
}

function InstanceList({ paneId, active, target }: ProductProps) {
    const view = useGcpView(paneId);
    const instances = useResourceEnabled(active, instancesR, ...keyOf(target));
    const words = view.filter.trim().toLowerCase();
    return (
        <div className="gcp-page">
            <PageHeader title="VM instances">
                <Button onClick={() => visit(consoleUrl("compute/instances", target.project))}>Create instance in Cloud console</Button>
            </PageHeader>
            <Filter paneId={paneId} placeholder="Filter VM instances" />
            <Loaded handle={instances} label="VM instances" empty={(rows) => (rows.length === 0 ? "No VM instances in this project." : null)}>
                {(rows) => (
                    <Table
                        label="VM instances"
                        rows={rows.filter((vm) => !words || vm.name.toLowerCase().includes(words) || vm.zone.includes(words))}
                        rowKey={(vm) => `${vm.zone}/${vm.name}`}
                        onOpen={(vm: VmInstance) => updateGcpView(paneId, { open: `${vm.zone}/${vm.name}`, tab: null })}
                        columns={[
                            { label: "Status", width: "70px", cell: (vm) => <Status state={vm.status} /> },
                            { label: "Name", width: "minmax(140px, 1.2fr)", cell: (vm) => <span className="gcp-name">{vm.name}</span> },
                            { label: "Zone", cell: (vm) => vm.zone },
                            { label: "Machine type", cell: (vm) => vm.machineType },
                            { label: "Internal IP", cell: (vm) => vm.internalIps.join(", ") || "—" },
                            { label: "External IP", cell: (vm) => vm.externalIps.join(", ") || "—" },
                            { label: "", width: "60px", cell: (vm) => (vm.spot ? <span className="gcp-chip">Spot</span> : null) },
                        ]}
                    />
                )}
            </Loaded>
        </div>
    );
}

const TABS = [
    { value: "details", label: "Details" },
    { value: "observability", label: "Observability" },
    { value: "serial", label: "Serial console" },
    { value: "logs", label: "Logs" },
] as const;

function InstancePage({
    paneId,
    active,
    target,
    zone,
    name,
    onBack,
}: {
    paneId: string;
    active: boolean;
    target: Target;
    zone: string;
    name: string;
    onBack: () => void;
}) {
    const view = useGcpView(paneId);
    const tab = TABS.find((each) => each.value === view.tab)?.value ?? "details";
    const vm = useResourceEnabled(active, instanceR, ...keyOf(target), zone, name);
    const action = useAction();
    const power = (change: Power) =>
        void action.run(
            {
                title: `${POWER[change].label} ${name}?`,
                body: POWER[change].body(name),
                confirmLabel: POWER[change].label,
                destructive: POWER[change].destructive,
            },
            `${POWER[change].label} sent to ${name}`,
            () => gcpApi.instancePower(target, zone, name, change),
        );
    return (
        <div className="gcp-page">
            <PageHeader title={name} back={{ label: "VM instances", go: onBack }}>
                {vm.data && (
                    <Button
                        primary
                        disabled={vm.data.status !== "RUNNING"}
                        title={vm.data.status === "RUNNING" ? "Opens gcloud compute ssh in a terminal" : "Start the VM first"}
                        onClick={() => void runInTerminal(`SSH ${name}`, vm.data!.ssh).catch(swallow("open SSH"))}>
                        SSH
                    </Button>
                )}
                {vm.data &&
                    powersFor(vm.data.status).map((change) => (
                        <Button key={change} danger={POWER[change].destructive} disabled={action.busy} onClick={() => power(change)}>
                            {POWER[change].label}
                        </Button>
                    ))}
            </PageHeader>
            <Loaded handle={vm} label={name} empty={() => null}>
                {(detail) => (
                    <>
                        <div className="gcp-summary">
                            <Status state={detail.status} text />
                            <span className="gcp-muted">{detail.zone}</span>
                            <span className="gcp-muted">{detail.machineType}</span>
                        </div>
                        <Tabs label={name} tabs={TABS} value={tab} onChange={(value) => updateGcpView(paneId, { tab: value })} />
                        {tab === "details" && <InstanceFacts detail={detail} />}
                        {tab === "observability" && <InstanceCpu active={active} target={target} id={detail.id} />}
                        {tab === "serial" && <Serial active={active} target={target} zone={zone} name={name} vm={detail} />}
                        {tab === "logs" && <LogStream active={active} target={target} query={{ resource: "instance", name: detail.id }} />}
                    </>
                )}
            </Loaded>
        </div>
    );
}

function InstanceFacts({ detail }: { detail: VmDetail }) {
    return (
        <div className="gcp-section">
            <Facts
                items={[
                    ["Instance id", <Copyable key="id" text={detail.id} what="the instance id" />],
                    ["Internal IP", detail.internalIps.join(", ") || "None"],
                    ["External IP", detail.externalIps.join(", ") || "None"],
                    ["Provisioning", detail.spot ? "Spot" : "Standard"],
                    ["Service account", detail.serviceAccount],
                    ["Created", ago(detail.createdAt)],
                    ["Last started", ago(detail.startedAt)],
                    ["SSH", <Copyable key="ssh" text={detail.ssh.join(" ")} what="the SSH command" />],
                ]}
            />
            <h2 className="gcp-section-title">Disks</h2>
            <Table
                label="Disks"
                rows={detail.disks}
                rowKey={(disk) => disk.name}
                columns={[
                    { label: "Name", cell: (disk) => disk.name },
                    { label: "Size", cell: (disk) => (disk.sizeGb === null ? "—" : size(disk.sizeGb * 1024 ** 3)) },
                    { label: "Type", cell: (disk) => (disk.boot ? "Boot disk" : "Additional disk") },
                    { label: "Mode", cell: (disk) => disk.mode?.replace("READ_", "Read ").replace("WRITE", "write").replace("ONLY", "only") ?? "—" },
                ]}
            />
        </div>
    );
}

function InstanceCpu({ active, target, id }: { active: boolean; target: Target; id: string }) {
    const key = keyOf(target);
    const hour = useResourceEnabled(active, metricsR, ...key, "instanceCpu", id, "1h");
    const day = useResourceEnabled(active, metricsR, ...key, "instanceCpu", id, "1d");
    const week = useResourceEnabled(active, metricsR, ...key, "instanceCpu", id, "7d");
    return (
        <div className="gcp-charts">
            <Sparkline title="CPU, last hour" series={hour.data} />
            <Sparkline title="CPU, last day" series={day.data} />
            <Sparkline title="CPU, last 7 days" series={week.data} />
        </div>
    );
}

function Serial({ active, target, zone, name, vm }: { active: boolean; target: Target; zone: string; name: string; vm: VmDetail }) {
    const serial = useResourceEnabled(active, serialR, ...keyOf(target), zone, name);
    return (
        <Loaded
            handle={serial}
            label="serial output"
            empty={(output) => (output.contents ? null : "The VM has printed nothing to its serial console.")}>
            {(output) => (
                <div className="gcp-section">
                    <div className="gcp-log-bar">
                        {output.trimmed && <span className="gcp-muted">Showing the last 256 KiB.</span>}
                        <Button onClick={() => void serial.refresh()}>Refresh</Button>
                        <SendToAgent delivery={() => serialForAgent(target.project, vm, output.contents)} />
                    </div>
                    <pre className="gcp-terminal">{output.contents}</pre>
                </div>
            )}
        </Loaded>
    );
}
