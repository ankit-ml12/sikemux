import { defineService } from "../../plugin-api/service";
import { awsApi } from "./api";
import { AWS_PLUGIN } from "./kinds";
import type { AwsBilling } from "./types.generated";

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

export const ec2InstancesR = aws.resource("ec2.instances", (profile: string) => awsApi.ec2Instances(profile), 60_000);

export const lambdaFnsR = aws.resource("lambda.functions", (profile: string) => awsApi.lambdaFunctions(profile), 60_000);

export const sqsQueuesR = aws.resource("sqs.queues", (profile: string) => awsApi.sqsQueues(profile), 60_000);

/** How many closed months the costs reach back over, beside the month in progress. */
const MONTHS_BACK = 5;

/** The account's costs by month, worked out by the plugin and kept by the core. */
export const billingV = aws.view<[profile: string], AwsBilling>("billingView", (profile) => ({ profile, monthsBack: MONTHS_BACK }));

export const s3BucketsR = aws.resource("s3.buckets", (profile: string) => awsApi.s3Buckets(profile), 5 * 60_000);
