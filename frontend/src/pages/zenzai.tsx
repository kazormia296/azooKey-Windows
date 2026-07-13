import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Bot, User, Cpu, Download, RefreshCw } from "lucide-react";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select"
import { useEffect, useState } from "react";
import { toast } from "sonner"
import { invoke } from '@tauri-apps/api/core';
import {
    Tooltip,
    TooltipContent,
    TooltipProvider,
    TooltipTrigger,
} from "@/components/ui/tooltip"

const ToolTipSelectItem = ({
    name,
    value,
    disabled,
    tooltip
}: {
    name: string;
    value: string;
    disabled: boolean;
    tooltip: string;
}) => {
    return (
        <TooltipProvider>
            <Tooltip>
                <TooltipTrigger>
                    <SelectItem value={value} disabled={disabled}>
                        {name}
                    </SelectItem>
                </TooltipTrigger>
                {disabled && <TooltipContent side="left">
                    {tooltip}
                </TooltipContent>}
            </Tooltip>
        </TooltipProvider>
    )
}

export const Zenzai = () => {
    const [value, setValue] = useState({
        enable: false,
        profile: "",
        backend: "",
    });

    const [capability, setCapability] = useState({
        cpu: true,
        cuda: false,
        vulkan: false,
    });
    const [modelReady, setModelReady] = useState<boolean | null>(null);
    const [modelDownloading, setModelDownloading] = useState(false);

    const refreshModelStatus = async () => {
        try {
            setModelReady(await invoke<boolean>("zenzai_model_status"));
        } catch {
            setModelReady(false);
        }
    };

    // Load config on component mount
    useEffect(() => {
        invoke<any>("get_config")
            .then((data) => {
                const zenzai = data.zenzai;
                setValue({
                    enable: zenzai.enable,
                    profile: zenzai.profile,
                    backend: zenzai.backend,
                });
            })
            .catch(() => {
                // Keep default values if config fetch fails
            });

        invoke("check_capability").then((capability: any) => {
            setCapability({
                cpu: capability["cpu"],
                cuda: capability["cuda"],
                vulkan: capability["vulkan"],
            });
        })
        refreshModelStatus();
    }, []);

    const downloadModel = async () => {
        if (modelDownloading) {
            return;
        }
        setModelDownloading(true);
        try {
            await invoke("download_zenzai_model");
            await refreshModelStatus();
            toast("Zenzaiモデルをダウンロードしました", {
                description: "モデルを再読み込みし、すぐに利用できる状態にしました",
            });
        } catch (error) {
            toast("Zenzaiモデルのダウンロードに失敗しました", {
                description: String(error),
            });
        } finally {
            setModelDownloading(false);
        }
    };

    const updateConfig = async (updater: (config: any) => void) => {
        try {
            const data = await invoke<any>("get_config");
            updater(data);
            await invoke("update_config", { newConfig: data });
            return data;
        } catch (error) {
            toast("設定の更新に失敗しました");
            return null;
        }
    };

    const handleZenzaiChange = async () => {
        const data = await updateConfig((data) => {
            data.zenzai.enable = !value.enable;
        });
        
        if (data) {
            setValue((prev) => ({ ...prev, enable: data.zenzai.enable }));
        }
    };

    const handleProfileChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
        const newProfile = event.target.value;
        setValue((prev) => ({ ...prev, profile: newProfile }));
        
        updateConfig((data) => {
            data.zenzai.profile = newProfile;
        });
    };

    const handleBackendChange = async (backend: string) => {
        const data = await updateConfig((data) => {
            data.zenzai.backend = backend;
        });
        
        if (data) {
            setValue((prev) => ({ ...prev, backend }));
            toast("バックエンドが変更されました", {
                description: "変更を適用するには、PCを再起動してください",
                duration: 10000,
            });
        }
    };

    return (
        <div className="space-y-8">
            <section className="space-y-2">
                <h1 className="text-sm font-bold text-foreground">Zenzai</h1>
                {modelReady !== true && (
                    <div className="flex items-center gap-4 rounded-md border border-amber-300 bg-amber-100 p-4 text-amber-950 dark:border-amber-700 dark:bg-amber-950/40 dark:text-amber-100">
                        <Download />
                        <div className="flex-1 space-y-1">
                            <p className="text-sm font-medium leading-none">
                                Zenzaiモデルが未インストールです
                            </p>
                            <p className="text-xs opacity-80">
                                共通のモデル配布リリースから取得し、ダウンロード後にIMEへ再読み込みします
                            </p>
                        </div>
                        <Button
                            variant="outline"
                            className="border-amber-400 bg-transparent text-amber-950 hover:bg-amber-200 dark:border-amber-600 dark:text-amber-100 dark:hover:bg-amber-900/60"
                            disabled={modelDownloading}
                            onClick={downloadModel}
                        >
                            {modelDownloading ? <RefreshCw className="animate-spin" /> : "ダウンロード"}
                        </Button>
                    </div>
                )}
                <div className="flex items-center space-x-4 rounded-md border p-4">
                    <Bot />
                    <div className="flex-1 space-y-1">
                        <p className="text-sm font-medium leading-none">
                            Zenzaiを有効化
                        </p>
                        <p className="text-xs text-muted-foreground">
                            Zenzaiを有効にして、変換精度を向上させます
                        </p>
                    </div>
                    <Switch checked={value.enable} onCheckedChange={handleZenzaiChange} />
                </div>
                <div className="space-y-4 rounded-md border p-4">
                    <div className="flex items-center space-x-4 ">
                        <User />
                        <div className="flex-1 space-y-1">
                            <p className="text-sm font-medium leading-none">
                                変換プロファイル
                            </p>
                            <p className="text-xs text-muted-foreground">
                                Zenzaiで利用されるユーザー情報を設定します
                            </p>
                        </div>
                    </div>
                    <Textarea placeholder="例）山田太郎、数学科の学生。" value={value.profile} disabled={!value.enable} onChange={handleProfileChange} />
                </div>
                <div className="flex items-center space-x-4 rounded-md border p-4">
                    <Cpu />
                    <div className="flex-1 space-y-1">
                        <p className="text-sm font-medium leading-none">
                            バックエンド
                        </p>
                        <p className="text-xs text-muted-foreground">
                            Zenzaiを利用するバックエンドを選択します
                        </p>
                    </div>
                    <Select disabled={!value.enable} value={value.backend} onValueChange={handleBackendChange}>
                        <SelectTrigger className="w-48">
                            <SelectValue placeholder="バックエンドを選択" />
                        </SelectTrigger>
                        <SelectContent className="flex flex-col">
                            <ToolTipSelectItem name="CPU (非推奨)" value="cpu" disabled={!capability.cpu} tooltip="" />
                            <ToolTipSelectItem name="CUDA (NVIDIA GPU)" value="cuda" disabled={!capability.cuda} tooltip="CUDA Toolkit 12をインストールする必要があります" />
                            <ToolTipSelectItem name="Vulkan" value="vulkan" disabled={!capability.vulkan} tooltip="お使いのPCはVulkanに対応していません" />
                        </SelectContent>
                    </Select>
                </div>
            </section>
        </div>
    )
}
