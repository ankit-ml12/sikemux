import { resource } from "../../plugin-api/resources";
import {
    dockerApi,
    type Container,
    type ContainerDetail,
    type ContainerStats,
    type Disk,
    type DockerStatus,
    type Image,
    type Stack,
    type Volume,
} from "./api";

export const dockerStatusR = resource({
    kind: "docker.status",
    fetch: (): Promise<DockerStatus> => dockerApi.status(),
    staleAfterMs: 10_000,
});

export const dockerContainersR = resource({
    kind: "docker.containers",
    fetch: (): Promise<Container[]> => dockerApi.containers(),
    staleAfterMs: 5_000,
});

export const dockerContainerR = resource({
    kind: "docker.container",
    fetch: (id: string): Promise<ContainerDetail> => dockerApi.container(id),
    staleAfterMs: 5_000,
});

export const dockerStatsR = resource({
    kind: "docker.stats",
    fetch: (): Promise<ContainerStats[]> => dockerApi.stats(),
    staleAfterMs: 3_000,
});

export const dockerStacksR = resource({
    kind: "docker.stacks",
    fetch: (): Promise<Stack[]> => dockerApi.stacks(),
    staleAfterMs: 5_000,
});

export const dockerImagesR = resource({
    kind: "docker.images",
    fetch: (): Promise<Image[]> => dockerApi.images(),
    staleAfterMs: 30_000,
});

export const dockerVolumesR = resource({
    kind: "docker.volumes",
    fetch: (): Promise<Volume[]> => dockerApi.volumes(),
    staleAfterMs: 30_000,
});

export const dockerDiskR = resource({
    kind: "docker.disk",
    fetch: (): Promise<Disk> => dockerApi.disk(),
    staleAfterMs: 60_000,
});
