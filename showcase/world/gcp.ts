type Params = Record<string, unknown>;

const minutesAgo = (minutes: number) =>
  new Date(Date.now() - minutes * 60_000).toISOString();

const STATUS = {
  installed: true,
  accounts: [
    { account: "edon@acme.dev", active: true, serviceAccount: false },
    {
      account: "deployer@acme-prod.iam.gserviceaccount.com",
      active: false,
      serviceAccount: true,
    },
  ],
  configurations: [
    {
      name: "default",
      active: true,
      account: "edon@acme.dev",
      project: "acme-prod",
      region: "us-central1",
      zone: "us-central1-a",
    },
  ],
  applicationDefault: true,
  message: null,
};

const PROJECTS = [
  {
    id: "acme-prod",
    name: "Acme Production",
    number: "481516234200",
    createdAt: minutesAgo(600_000),
  },
  {
    id: "acme-staging",
    name: "Acme Staging",
    number: "481516234201",
    createdAt: minutesAgo(500_000),
  },
  {
    id: "acme-data",
    name: "Acme Data",
    number: "481516234202",
    createdAt: minutesAgo(400_000),
  },
];

const service = (
  name: string,
  state: string,
  minutes: number,
  by: string,
  traffic: unknown[],
  image: string,
) => ({
  name,
  location: "us-central1",
  url: `https://${name}-x7k2q-uc.a.run.app`,
  state,
  message:
    state === "FAILED"
      ? "Revision 'checkout-00031-qzm' is not ready: the container failed to start and listen on port 8080."
      : null,
  latestRevision: `${name}-00031-qzm`,
  image,
  cpu: "1",
  memory: "512Mi",
  minInstances: 0,
  maxInstances: 20,
  concurrency: 80,
  serviceAccount: `${name}@acme-prod.iam.gserviceaccount.com`,
  ingress: "INGRESS_TRAFFIC_ALL",
  traffic,
  updatedAt: minutesAgo(minutes),
  updatedBy: by,
});

const SERVICES = [
  service(
    "storefront",
    "READY",
    42,
    "lena@acme.dev",
    [
      {
        revision: "storefront-00031-qzm",
        latest: true,
        percent: 100,
        tag: null,
        url: null,
      },
    ],
    "us-docker.pkg.dev/acme-prod/web/storefront:2.4.0",
  ),
  service(
    "api",
    "READY",
    180,
    "edon@acme.dev",
    [
      {
        revision: "api-00058-bxt",
        latest: false,
        percent: 90,
        tag: null,
        url: null,
      },
      {
        revision: "api-00059-kfn",
        latest: false,
        percent: 10,
        tag: "canary",
        url: "https://canary---api-x7k2q-uc.a.run.app",
      },
    ],
    "us-docker.pkg.dev/acme-prod/api/api:1.18.2",
  ),
  service(
    "checkout",
    "FAILED",
    7,
    "maya@acme.dev",
    [
      {
        revision: "checkout-00030-hhd",
        latest: false,
        percent: 100,
        tag: null,
        url: null,
      },
    ],
    "us-docker.pkg.dev/acme-prod/web/checkout:0.9.1",
  ),
  service(
    "image-resizer",
    "DEPLOYING",
    1,
    "deployer@acme-prod.iam.gserviceaccount.com",
    [{ revision: null, latest: true, percent: 100, tag: null, url: null }],
    "us-docker.pkg.dev/acme-prod/media/resizer:3.1.0",
  ),
];

const REVISIONS = [
  {
    name: "api-00059-kfn",
    state: "READY",
    image: "us-docker.pkg.dev/acme-prod/api/api:1.18.2",
    createdAt: minutesAgo(180),
    percent: 10,
    tags: ["canary"],
  },
  {
    name: "api-00058-bxt",
    state: "READY",
    image: "us-docker.pkg.dev/acme-prod/api/api:1.18.1",
    createdAt: minutesAgo(2900),
    percent: 90,
    tags: [],
  },
  {
    name: "api-00057-mqa",
    state: "READY",
    image: "us-docker.pkg.dev/acme-prod/api/api:1.18.0",
    createdAt: minutesAgo(8600),
    percent: 0,
    tags: [],
  },
  {
    name: "api-00056-wcz",
    state: "FAILED",
    image: "us-docker.pkg.dev/acme-prod/api/api:1.17.9",
    createdAt: minutesAgo(12000),
    percent: 0,
    tags: [],
  },
];

const INSTANCES = [
  {
    id: "4712230118",
    name: "bastion",
    zone: "us-central1-a",
    machineType: "e2-micro",
    status: "RUNNING",
    internalIps: ["10.128.0.2"],
    externalIps: ["34.123.45.67"],
    disks: [{ name: "bastion", sizeGb: 10, boot: true, mode: "READ_WRITE" }],
    spot: false,
    serviceAccount: null,
    createdAt: minutesAgo(200_000),
    startedAt: minutesAgo(9000),
  },
  {
    id: "4712230119",
    name: "ml-trainer-1",
    zone: "us-central1-b",
    machineType: "n1-standard-8",
    status: "RUNNING",
    internalIps: ["10.128.0.14"],
    externalIps: [],
    disks: [
      { name: "ml-trainer-1", sizeGb: 200, boot: true, mode: "READ_WRITE" },
      { name: "datasets", sizeGb: 1000, boot: false, mode: "READ_ONLY" },
    ],
    spot: true,
    serviceAccount: "trainer@acme-prod.iam.gserviceaccount.com",
    createdAt: minutesAgo(20_000),
    startedAt: minutesAgo(300),
  },
  {
    id: "4712230120",
    name: "legacy-cron",
    zone: "europe-west1-b",
    machineType: "e2-small",
    status: "TERMINATED",
    internalIps: ["10.132.0.3"],
    externalIps: [],
    disks: [
      { name: "legacy-cron", sizeGb: 20, boot: true, mode: "READ_WRITE" },
    ],
    spot: false,
    serviceAccount: null,
    createdAt: minutesAgo(400_000),
    startedAt: minutesAgo(40_000),
  },
];

const CLUSTERS = [
  {
    name: "prod-gke",
    location: "us-central1",
    status: "RUNNING",
    message: null,
    version: "1.30.5-gke.1014000",
    nodeVersion: "1.30.5-gke.1014000",
    nodes: 6,
    autopilot: false,
    channel: "REGULAR",
    network: "default",
    private: false,
    nodePools: [
      {
        name: "default-pool",
        status: "RUNNING",
        version: "1.30.5-gke.1014000",
        machineType: "e2-standard-4",
        diskSizeGb: 100,
        spot: false,
        nodes: 3,
        autoscaling: true,
        minNodes: 1,
        maxNodes: 5,
      },
      {
        name: "spot-pool",
        status: "RUNNING",
        version: "1.30.5-gke.1014000",
        machineType: "e2-standard-8",
        diskSizeGb: 100,
        spot: true,
        nodes: 3,
        autoscaling: false,
        minNodes: null,
        maxNodes: null,
      },
    ],
    createdAt: minutesAgo(300_000),
  },
  {
    name: "batch-autopilot",
    location: "europe-west1",
    status: "RUNNING",
    message: null,
    version: "1.31.1-gke.1678000",
    nodeVersion: null,
    nodes: 2,
    autopilot: true,
    channel: "RAPID",
    network: "default",
    private: true,
    nodePools: [],
    createdAt: minutesAgo(90_000),
  },
];

const WORKLOADS = [
  {
    kind: "Deployment",
    name: "orders-api",
    namespace: "shop",
    desired: 3,
    ready: 3,
    containers: [
      { name: "api", image: "us-docker.pkg.dev/acme-prod/shop/orders:4.2.0" },
    ],
    selector: "app=orders-api",
    createdAt: minutesAgo(40_000),
  },
  {
    kind: "Deployment",
    name: "payments",
    namespace: "shop",
    desired: 2,
    ready: 1,
    containers: [
      {
        name: "payments",
        image: "us-docker.pkg.dev/acme-prod/shop/payments:2.0.3",
      },
    ],
    selector: "app=payments",
    createdAt: minutesAgo(9000),
  },
  {
    kind: "StatefulSet",
    name: "redis",
    namespace: "shop",
    desired: 1,
    ready: 1,
    containers: [{ name: "redis", image: "redis:7.4" }],
    selector: "app=redis",
    createdAt: minutesAgo(200_000),
  },
  {
    kind: "DaemonSet",
    name: "otel-agent",
    namespace: "observability",
    desired: 6,
    ready: 6,
    containers: [
      { name: "otel", image: "otel/opentelemetry-collector:0.110.0" },
    ],
    selector: "app=otel-agent",
    createdAt: minutesAgo(100_000),
  },
];

const PODS = [
  {
    name: "orders-api-6d9f7b8c4-2xkqp",
    namespace: "shop",
    phase: "Running",
    reason: null,
    ready: "1/1",
    restarts: 0,
    node: "gke-prod-default-pool-a1",
    ip: "10.4.1.12",
    containers: [{ name: "api", image: "orders:4.2.0" }],
    createdAt: minutesAgo(3000),
  },
  {
    name: "payments-5c8d6f7d9-r7mzw",
    namespace: "shop",
    phase: "Running",
    reason: "CrashLoopBackOff",
    ready: "0/1",
    restarts: 14,
    node: "gke-prod-spot-pool-b3",
    ip: "10.4.2.31",
    containers: [{ name: "payments", image: "payments:2.0.3" }],
    createdAt: minutesAgo(90),
  },
  {
    name: "payments-5c8d6f7d9-zq4lt",
    namespace: "shop",
    phase: "Running",
    reason: null,
    ready: "1/1",
    restarts: 0,
    node: "gke-prod-default-pool-a2",
    ip: "10.4.1.40",
    containers: [{ name: "payments", image: "payments:2.0.3" }],
    createdAt: minutesAgo(9000),
  },
];

const BUCKETS = [
  {
    name: "acme-prod-assets",
    location: "US",
    locationType: "multi-region",
    storageClass: "STANDARD",
    publicAccessPrevention: "inherited",
    uniformAccess: true,
    versioning: false,
    createdAt: minutesAgo(500_000),
  },
  {
    name: "acme-prod-backups",
    location: "US-CENTRAL1",
    locationType: "region",
    storageClass: "NEARLINE",
    publicAccessPrevention: "enforced",
    uniformAccess: true,
    versioning: true,
    createdAt: minutesAgo(400_000),
  },
  {
    name: "acme-prod_cloudbuild",
    location: "US",
    locationType: "multi-region",
    storageClass: "STANDARD",
    publicAccessPrevention: "enforced",
    uniformAccess: true,
    versioning: false,
    createdAt: minutesAgo(300_000),
  },
];

const SQL = [
  {
    name: "orders-db",
    engine: "POSTGRES_16",
    region: "us-central1",
    zone: "us-central1-c",
    state: "RUNNING",
    tier: "db-custom-2-7680",
    diskGb: 100,
    highAvailability: true,
    backups: true,
    backupWindow: "03:00",
    publicIp: null,
    privateIp: "10.20.0.3",
    connectionName: "acme-prod:us-central1:orders-db",
    createdAt: minutesAgo(300_000),
  },
  {
    name: "analytics-mysql",
    engine: "MYSQL_8_0",
    region: "us-east1",
    zone: "us-east1-b",
    state: "STOPPED",
    tier: "db-n1-standard-1",
    diskGb: 50,
    highAvailability: false,
    backups: false,
    backupWindow: null,
    publicIp: "34.75.10.20",
    privateIp: null,
    connectionName: "acme-prod:us-east1:analytics-mysql",
    createdAt: minutesAgo(200_000),
  },
];

const build = (
  id: string,
  status: string,
  minutes: number,
  trigger: string,
  branch: string,
  commit: string,
  failure: string | null = null,
) => ({
  id,
  status,
  detail: failure ? "Build step failure" : null,
  failure,
  trigger,
  triggerId: "trig-1",
  repo: "acme/storefront",
  branch,
  commit,
  steps: [
    {
      id: "install",
      image: "node:20",
      status: "SUCCESS",
      startedAt: minutesAgo(minutes),
      endedAt: minutesAgo(minutes - 1),
    },
    {
      id: "test",
      image: "node:20",
      status: status === "WORKING" ? "WORKING" : "SUCCESS",
      startedAt: minutesAgo(minutes - 1),
      endedAt: status === "WORKING" ? null : minutesAgo(minutes - 2),
    },
    {
      id: "deploy",
      image: "gcr.io/google.com/cloudsdktool/cloud-sdk",
      status: failure ? "FAILURE" : status === "WORKING" ? "QUEUED" : "SUCCESS",
      startedAt: status === "WORKING" ? null : minutesAgo(minutes - 2),
      endedAt: status === "WORKING" ? null : minutesAgo(minutes - 3),
    },
  ],
  images: ["us-docker.pkg.dev/acme-prod/web/storefront:2.4.0"],
  createdAt: minutesAgo(minutes),
  startedAt: minutesAgo(minutes),
  finishedAt: status === "WORKING" ? null : minutesAgo(minutes - 3),
  logUrl: null,
  region: null,
});

const BUILDS = [
  build(
    "7c1d2e3f-4a5b-4c6d-8e9f-0a1b2c3d4e5f",
    "WORKING",
    2,
    "deploy-main",
    "main",
    "9f1c2ab0e4",
  ),
  build(
    "6b0c1d2e-3f4a-4b5c-9d8e-7f6a5b4c3d2e",
    "FAILURE",
    35,
    "deploy-main",
    "main",
    "4be71d03aa",
    "Step #2 - \"deploy\": ERROR: (gcloud.run.deploy) PERMISSION_DENIED: Permission 'run.services.update' denied",
  ),
  build(
    "5a9b0c1d-2e3f-4a4b-8c7d-6e5f4a3b2c1d",
    "SUCCESS",
    190,
    "deploy-main",
    "main",
    "c03e9d41f7",
  ),
  build(
    "4f8a9b0c-1d2e-4f3a-9b8c-7d6e5f4a3b2c",
    "SUCCESS",
    1400,
    "pr-checks",
    "search-filters",
    "b28c11e6d0",
  ),
];

const BUILD_LOG = [
  'starting build "6b0c1d2e-3f4a-4b5c-9d8e-7f6a5b4c3d2e"',
  "FETCHSOURCE",
  "From https://github.com/acme/storefront",
  " * branch            4be71d03aa -> FETCH_HEAD",
  'Step #0 - "install": added 812 packages in 24s',
  'Step #1 - "test": Test Suites: 48 passed, 48 total',
  'Step #1 - "test": Tests:       612 passed, 612 total',
  'Step #2 - "deploy": Deploying container to Cloud Run service [storefront] in project [acme-prod] region [us-central1]',
  "Step #2 - \"deploy\": ERROR: (gcloud.run.deploy) PERMISSION_DENIED: Permission 'run.services.update' denied on resource 'namespaces/acme-prod/services/storefront' (or it may not exist).",
  'Finished Step #2 - "deploy"',
  'ERROR: build step 2 "gcr.io/google.com/cloudsdktool/cloud-sdk" failed: step exited with non-zero status: 1',
];

const LOGS = [
  {
    severity: "ERROR",
    text: "Error: connect ECONNREFUSED 10.20.0.3:5432 — orders-db is not accepting connections",
    minutes: 4,
  },
  {
    severity: "WARNING",
    text: "Slow query: SELECT * FROM orders WHERE customer_id = $1 took 2.4s",
    minutes: 9,
  },
  { severity: "INFO", text: "GET /api/orders/18234 200 48ms", minutes: 11 },
  {
    severity: "ERROR",
    text: "Unhandled rejection: TypeError: Cannot read properties of undefined (reading 'taxRate')",
    minutes: 18,
  },
  {
    severity: "INFO",
    text: "Started storefront revision storefront-00031-qzm, listening on :8080",
    minutes: 42,
  },
  {
    severity: "NOTICE",
    text: "google.cloud.run.v2.Services.UpdateService · lena@acme.dev",
    minutes: 43,
  },
];

const entry = (raw: (typeof LOGS)[number], index: number) => ({
  id: `e${index}-${raw.minutes}`,
  at: minutesAgo(raw.minutes),
  severity: raw.severity,
  text: raw.text,
  log: "run.googleapis.com/stderr",
  resource: "cloud_run_revision",
  labels: { service_name: "storefront", location: "us-central1" },
  http: raw.text.startsWith("GET")
    ? {
        method: "GET",
        url: "/api/orders/18234",
        status: 200,
        latency: "0.048s",
      }
    : null,
  json:
    raw.severity === "ERROR"
      ? {
          message: raw.text,
          stack: "at Totals (/app/checkout/totals.js:42:18)",
        }
      : null,
  trace: null,
});

const series = (base: number, swing: number, unit: string) => ({
  unit,
  step: 180,
  points: Array.from({ length: 60 }, (_, index) => ({
    at: minutesAgo((60 - index) * 6),
    value: Math.max(
      0,
      base + Math.sin(index / 5) * swing + ((index * 7) % 5) * swing * 0.1,
    ),
  })),
});

export const GCP_LOG_LINES = LOGS.map(entry);

export const GCP: Record<string, (params: Params) => unknown> = {
  status: () => STATUS,
  projects: () => PROJECTS,
  agentPicks: () => ({
    project: { account: "edon@acme.dev", project: "acme-staging" },
    default: { account: null, project: "acme-prod" },
    effective: {
      pick: { account: "edon@acme.dev", project: "acme-staging" },
      source: "project",
    },
  }),
  pickForAgents: () => ({ project: null, default: null, effective: null }),
  runServices: () => SERVICES,
  runService: ({ service: name }) => ({
    ...(SERVICES.find((each) => each.name === name) ?? SERVICES[1]),
    revisions: REVISIONS,
    env: [
      { name: "NODE_ENV", secret: null },
      { name: "DATABASE_URL", secret: "orders-db-url" },
      { name: "STRIPE_SECRET_KEY", secret: "stripe-secret-key" },
    ],
  }),
  runSetTraffic: () => ({ operation: "op-traffic" }),
  runJobs: () => [
    {
      name: "nightly-export",
      location: "us-central1",
      state: "READY",
      image: "us-docker.pkg.dev/acme-prod/jobs/export:1.2",
      tasks: 4,
      executions: 212,
      latestExecution: "nightly-export-8x2kq",
      latestExecutionAt: minutesAgo(480),
      updatedAt: minutesAgo(20_000),
    },
  ],
  runJob: () => ({
    name: "nightly-export",
    location: "us-central1",
    state: "READY",
    image: "us-docker.pkg.dev/acme-prod/jobs/export:1.2",
    tasks: 4,
    executions: 212,
    latestExecution: "nightly-export-8x2kq",
    latestExecutionAt: minutesAgo(480),
    updatedAt: minutesAgo(20_000),
    recent: [
      {
        name: "nightly-export-8x2kq",
        state: "SUCCEEDED",
        tasks: 4,
        succeeded: 4,
        failed: 0,
        running: 0,
        startedAt: minutesAgo(480),
        completedAt: minutesAgo(471),
        createdAt: minutesAgo(480),
      },
      {
        name: "nightly-export-7w1jp",
        state: "FAILED",
        tasks: 4,
        succeeded: 3,
        failed: 1,
        running: 0,
        startedAt: minutesAgo(1920),
        completedAt: minutesAgo(1905),
        createdAt: minutesAgo(1920),
      },
    ],
  }),
  runJobExecute: () => ({ operation: "op-run" }),
  functions: () => [
    {
      name: "resize-upload",
      location: "us-central1",
      state: "ACTIVE",
      generation: "2nd gen",
      runtime: "nodejs20",
      entryPoint: "resize",
      url: "https://resize-upload-x7k2q-uc.a.run.app",
      memory: "512M",
      timeoutSeconds: 60,
      maxInstances: 50,
      serviceAccount: null,
      runService: "resize-upload",
      trigger: {
        kind: "event",
        event: "google.cloud.storage.object.v1.finalized",
        topic: null,
        retry: true,
      },
      env: ["BUCKET"],
      secretEnv: ["CDN_TOKEN"],
      updatedAt: minutesAgo(9000),
      message: null,
    },
    {
      name: "stripe-webhook",
      location: "us-central1",
      state: "ACTIVE",
      generation: "1st gen",
      runtime: "python312",
      entryPoint: "handle",
      url: "https://us-central1-acme-prod.cloudfunctions.net/stripe-webhook",
      memory: "256M",
      timeoutSeconds: 30,
      maxInstances: 10,
      serviceAccount: null,
      runService: null,
      trigger: { kind: "http" },
      env: [],
      secretEnv: ["STRIPE_WEBHOOK_SECRET"],
      updatedAt: minutesAgo(30_000),
      message: null,
    },
  ],
  instances: () => INSTANCES,
  instance: ({ instance: name, zone }) => ({
    ...(INSTANCES.find((each) => each.name === name) ?? INSTANCES[0]),
    ssh: [
      "gcloud",
      "compute",
      "ssh",
      String(name),
      `--zone=${String(zone)}`,
      "--project=acme-prod",
    ],
  }),
  instancePower: () => ({ operation: "op-power" }),
  instanceSerial: () => ({
    contents:
      "[    0.000000] Linux version 6.1.0-26-cloud-amd64\n[    1.204311] systemd[1]: Detected virtualization kvm.\n[  OK  ] Started google-guest-agent.service - Google Guest Agent.\n[  OK  ] Reached target multi-user.target - Multi-User System.\nml-trainer-1 login:",
    start: 0,
    next: 4096,
    trimmed: false,
  }),
  clusters: () => CLUSTERS,
  workloads: () => WORKLOADS,
  pods: ({ selector }) =>
    selector
      ? PODS.filter((pod) =>
          pod.name.startsWith(String(selector).split("=")[1] ?? ""),
        )
      : PODS,
  kubeServices: () => [
    {
      name: "orders-api",
      namespace: "shop",
      type: "ClusterIP",
      clusterIp: "10.8.4.21",
      externalIps: [],
      ports: ["80/TCP"],
    },
    {
      name: "storefront-lb",
      namespace: "shop",
      type: "LoadBalancer",
      clusterIp: "10.8.4.40",
      externalIps: ["35.202.11.90"],
      ports: ["443/TCP"],
    },
  ],
  podLogs: () => ({
    lines: [
      { at: minutesAgo(3), text: "payments starting on :8080" },
      { at: minutesAgo(3), text: "connecting to orders-db at 10.20.0.3:5432" },
      {
        at: minutesAgo(2),
        text: "panic: dial tcp 10.20.0.3:5432: connect: connection refused",
      },
    ],
  }),
  deploymentRestart: () => WORKLOADS[1],
  deploymentScale: () => ({ replicas: 3 }),
  nodePoolResize: () => ({ operation: "op-resize" }),
  buckets: () => BUCKETS,
  objects: ({ prefix }) =>
    prefix
      ? {
          bucket: "acme-prod-assets",
          prefix,
          folders: [],
          objects: [
            {
              name: `${String(prefix)}hero.webp`,
              size: 182_311,
              contentType: "image/webp",
              storageClass: "STANDARD",
              updatedAt: minutesAgo(9000),
            },
            {
              name: `${String(prefix)}robots.txt`,
              size: 64,
              contentType: "text/plain",
              storageClass: "STANDARD",
              updatedAt: minutesAgo(90_000),
            },
          ],
          more: false,
        }
      : {
          bucket: "acme-prod-assets",
          prefix: "",
          folders: ["images/", "exports/"],
          objects: [
            {
              name: "manifest.json",
              size: 2048,
              contentType: "application/json",
              storageClass: "STANDARD",
              updatedAt: minutesAgo(120),
            },
          ],
          more: false,
        },
  objectPreview: ({ object }) => ({
    name: object,
    size: 2048,
    contentType: "application/json",
    storageClass: "STANDARD",
    updatedAt: minutesAgo(120),
    bucket: "acme-prod-assets",
    preview: '{\n  "version": "2.4.0",\n  "assets": ["images/hero.webp"]\n}',
    trimmed: false,
    uri: `gs://acme-prod-assets/${String(object)}`,
  }),
  objectDownload: ({ object }) => ({
    path: `/Users/edon/Downloads/${String(object).split("/").pop()}`,
    size: 2048,
  }),
  sqlInstances: () => SQL,
  sqlInstance: ({ instance }) => ({
    ...(SQL.find((each) => each.name === instance) ?? SQL[0]),
    databases: [
      { name: "orders", charset: "UTF8" },
      { name: "postgres", charset: "UTF8" },
    ],
    users: [
      { name: "postgres", host: null, type: null },
      {
        name: "orders-api@acme-prod.iam",
        host: null,
        type: "CLOUD_IAM_SERVICE_ACCOUNT",
      },
    ],
    recentBackups: [
      {
        id: "1729220400000",
        status: "SUCCESSFUL",
        type: "AUTOMATED",
        startedAt: minutesAgo(600),
        endedAt: minutesAgo(596),
      },
    ],
    connect: [
      "gcloud",
      "sql",
      "connect",
      String(instance),
      "--project=acme-prod",
    ],
  }),
  sqlChange: () => ({ operation: "op-sql" }),
  topics: () => ({
    topics: [
      { name: "orders", retention: null },
      { name: "orders-dead-letter", retention: "604800s" },
      { name: "uploads", retention: null },
    ],
    subscriptions: [
      {
        name: "orders-fulfilment",
        topic: "orders",
        kind: "pull",
        pushEndpoint: null,
        ackDeadlineSeconds: 30,
        retention: "604800s",
        deadLetterTopic: "orders-dead-letter",
        maxDeliveryAttempts: 5,
        filter: null,
        ordering: true,
        state: "ACTIVE",
      },
      {
        name: "orders-to-bigquery",
        topic: "orders",
        kind: "bigquery",
        pushEndpoint: null,
        ackDeadlineSeconds: 10,
        retention: null,
        deadLetterTopic: null,
        maxDeliveryAttempts: null,
        filter: null,
        ordering: false,
        state: "ACTIVE",
      },
      {
        name: "uploads-webhook",
        topic: "uploads",
        kind: "push",
        pushEndpoint: "https://api.acme.dev/hooks/uploads",
        ackDeadlineSeconds: 20,
        retention: null,
        deadLetterTopic: null,
        maxDeliveryAttempts: null,
        filter: 'attributes.kind = "image"',
        ordering: false,
        state: "ACTIVE",
      },
    ],
  }),
  publish: () => ({ messageId: "11934582007142" }),
  peek: () => [
    {
      id: "11934581993212",
      publishedAt: minutesAgo(1),
      data: { text: '{"orderId": 18234, "status": "paid"}' },
      attributes: { kind: "order.paid" },
      deliveryAttempt: 1,
    },
    {
      id: "11934581993019",
      publishedAt: minutesAgo(3),
      data: { text: '{"orderId": 18233, "status": "paid"}' },
      attributes: { kind: "order.paid" },
      deliveryAttempt: 4,
    },
  ],
  builds: () => BUILDS,
  build: ({ id }) => ({
    ...(BUILDS.find((each) => each.id === id) ?? BUILDS[1]),
    log: BUILD_LOG,
    logSkipped: 0,
  }),
  buildChange: () => BUILDS[0],
  triggers: () => [
    {
      id: "trig-1",
      name: "deploy-main",
      description: "Build and deploy storefront on every push to main",
      repo: "acme/storefront",
      branch: "^main$",
      pullRequests: false,
      file: "cloudbuild.yaml",
      disabled: false,
    },
    {
      id: "trig-2",
      name: "pr-checks",
      description: null,
      repo: "acme/storefront",
      branch: ".*",
      pullRequests: true,
      file: "cloudbuild.pr.yaml",
      disabled: false,
    },
  ],
  triggerRun: () => BUILDS[0],
  secrets: () => [
    {
      name: "orders-db-url",
      replication: "automatic",
      createdAt: minutesAgo(300_000),
      expiresAt: null,
      nextRotation: null,
      rotationPeriod: null,
    },
    {
      name: "stripe-secret-key",
      replication: ["us-central1", "us-east1"],
      createdAt: minutesAgo(200_000),
      expiresAt: null,
      nextRotation: minutesAgo(-14_400),
      rotationPeriod: "2592000s",
    },
    {
      name: "cdn-token",
      replication: "automatic",
      createdAt: minutesAgo(90_000),
      expiresAt: null,
      nextRotation: null,
      rotationPeriod: null,
    },
  ],
  secret: ({ secret }) => ({
    name: secret,
    replication: "automatic",
    createdAt: minutesAgo(200_000),
    expiresAt: null,
    nextRotation: null,
    rotationPeriod: "2592000s",
    versions: [
      {
        version: "4",
        state: "ENABLED",
        createdAt: minutesAgo(20_000),
        destroyedAt: null,
      },
      {
        version: "3",
        state: "DISABLED",
        createdAt: minutesAgo(60_000),
        destroyedAt: null,
      },
      {
        version: "2",
        state: "DESTROYED",
        createdAt: minutesAgo(120_000),
        destroyedAt: minutesAgo(100_000),
      },
    ],
  }),
  secretVersion: () => ({}),
  logs: () => [...GCP_LOG_LINES],
  metrics: ({ chart }) => {
    const shapes: Record<string, [number, number, string]> = {
      runRequests: [42, 18, "req/s"],
      runErrors: [0.6, 0.5, "req/s"],
      runLatency: [180, 60, "ms"],
      runInstances: [4, 2, "instances"],
      instanceCpu: [38, 20, "%"],
      sqlCpu: [22, 10, "%"],
      sqlMemory: [61, 4, "%"],
      sqlConnections: [48, 12, "connections"],
      subscriptionBacklog: [120, 80, "messages"],
      subscriptionOldest: [14, 10, "s"],
      functionExecutions: [3, 2, "calls/s"],
    };
    const [base, swing, unit] = shapes[String(chart)] ?? [1, 1, ""];
    return series(base, swing, unit);
  },
};
