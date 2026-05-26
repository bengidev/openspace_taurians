import WorkspaceLayout from "@/components/WorkspaceLayout";

export default function WorkspaceRootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <WorkspaceLayout>{children}</WorkspaceLayout>;
}
