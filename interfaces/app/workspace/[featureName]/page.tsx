import { FeaturePanelContent } from "./FeaturePanelContent";

const FEATURES = ["editor", "terminal", "chat", "git", "settings"];

export function generateStaticParams() {
  return FEATURES.map((featureName) => ({ featureName }));
}

export default async function FeaturePanelPage({
  params,
}: {
  params: Promise<{ featureName: string }>;
}) {
  const { featureName } = await params;
  return <FeaturePanelContent featureName={featureName} />;
}
