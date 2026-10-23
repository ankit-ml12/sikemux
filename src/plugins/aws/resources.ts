import { defineService } from "../../plugin-api/service";
import { awsApi } from "./api";
import { AWS_PLUGIN } from "./kinds";
import type { AwsBilling, AwsEc2, AwsLambda, AwsS3, AwsSqs } from "./types.generated";

export const aws = defineService(AWS_PLUGIN, () => awsApi.status());

export const awsProfileR = aws.resource("profile", () => awsApi.profile());

export const awsProfilesR = aws.resource("profiles", () => awsApi.profiles());

export const awsAgentProfileR = aws.resource("agentProfile", (project: string | null) => awsApi.agentProfilePicks(project));

export const awsSetupR = aws.resource("setup", () => awsApi.setup());

export const awsIdentityR = aws.resource("identity", (profile: string, force: boolean) => awsApi.identity(profile, force), 60_000);

export const ecsClustersR = aws.resource("ecs.clusters", (profile: string) => awsApi.ecsClusters(profile), 30_000);

export const ecsServicesR = aws.resource("ecs.services", (profile: string, cluster: string) => awsApi.ecsServices(profile, cluster), 30_000);

export const ecsTasksR = aws.resource(
    "ecs.tasks",
    (profile: string, cluster: string, service: string) => awsApi.ecsTasks(profile, cluster, service),
    15_000,
);

export const ecsServiceLogConfigR = aws.resource("ecs.serviceLogConfig", (profile: string, cluster: string, service: string) =>
    awsApi.ecsServiceLogConfig(profile, cluster, service),
);

/** How many closed months the costs reach back over, beside the month in progress. */
const MONTHS_BACK = 5;

/** The account's costs by month, worked out by the plugin and kept by the core. */
export const billingV = aws.view<[profile: string], AwsBilling>("billingView", (profile) => ({ profile, monthsBack: MONTHS_BACK }));

export const ec2V = aws.view<[profile: string], AwsEc2>("ec2View", (profile) => ({ profile }));

export const lambdaV = aws.view<[profile: string], AwsLambda>("lambdaView", (profile) => ({ profile }));

export const sqsV = aws.view<[profile: string], AwsSqs>("sqsView", (profile) => ({ profile }));

export const s3V = aws.view<[profile: string], AwsS3>("s3View", (profile) => ({ profile }));
