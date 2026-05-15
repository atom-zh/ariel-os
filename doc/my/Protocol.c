#include <stdint.h>
#include <string.h>
#include "DisPatch.h"
#include "Protocol.h"
#include "GPS.h"
#include "iic_fram.h"
#include "TboxPlat.h"
#include "tbox_platform.h"
#include "tbox_statedata.h"

int upload_send_change_flag;
static  uint32_t protocol_key = 0x5CADF34E;//秘钥
extern uint8_t lock_vehicle;
extern uint8_t g_lock_power;
extern uint32_t csr;
extern LOG_MANAGER his_data;
const unsigned char auchCRCHi[] = {
0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41, 0x01, 0xC0,
0x80, 0x41, 0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41,
0x00, 0xC1, 0x81, 0x40, 0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0,
0x80, 0x41, 0x01, 0xC0, 0x80, 0x41, 0x00, 0xC1, 0x81, 0x40,
0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41, 0x00, 0xC1,
0x81, 0x40, 0x01, 0xC0, 0x80, 0x41, 0x01, 0xC0, 0x80, 0x41,
0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41, 0x00, 0xC1,
0x81, 0x40, 0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41,
0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41, 0x01, 0xC0,
0x80, 0x41, 0x00, 0xC1, 0x81, 0x40, 0x00, 0xC1, 0x81, 0x40,
0x01, 0xC0, 0x80, 0x41, 0x01, 0xC0, 0x80, 0x41, 0x00, 0xC1,
0x81, 0x40, 0x01, 0xC0, 0x80, 0x41, 0x00, 0xC1, 0x81, 0x40,
0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41, 0x01, 0xC0,
0x80, 0x41, 0x00, 0xC1, 0x81, 0x40, 0x00, 0xC1, 0x81, 0x40,
0x01, 0xC0, 0x80, 0x41, 0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0,
0x80, 0x41, 0x01, 0xC0, 0x80, 0x41, 0x00, 0xC1, 0x81, 0x40,
0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41, 0x01, 0xC0,
0x80, 0x41, 0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41,
0x00, 0xC1, 0x81, 0x40, 0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0,
0x80, 0x41, 0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41,
0x01, 0xC0, 0x80, 0x41, 0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0,
0x80, 0x41, 0x00, 0xC1, 0x81, 0x40, 0x00, 0xC1, 0x81, 0x40,
0x01, 0xC0, 0x80, 0x41, 0x01, 0xC0, 0x80, 0x41, 0x00, 0xC1,
0x81, 0x40, 0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41,
0x00, 0xC1, 0x81, 0x40, 0x01, 0xC0, 0x80, 0x41, 0x01, 0xC0,
0x80, 0x41, 0x00, 0xC1, 0x81, 0x40
} ;

const unsigned char  auchCRCLo[] = {
0x00, 0xC0, 0xC1, 0x01, 0xC3, 0x03, 0x02, 0xC2, 0xC6, 0x06,
0x07, 0xC7, 0x05, 0xC5, 0xC4, 0x04, 0xCC, 0x0C, 0x0D, 0xCD,
0x0F, 0xCF, 0xCE, 0x0E, 0x0A, 0xCA, 0xCB, 0x0B, 0xC9, 0x09,
0x08, 0xC8, 0xD8, 0x18, 0x19, 0xD9, 0x1B, 0xDB, 0xDA, 0x1A,
0x1E, 0xDE, 0xDF, 0x1F, 0xDD, 0x1D, 0x1C, 0xDC, 0x14, 0xD4,
0xD5, 0x15, 0xD7, 0x17, 0x16, 0xD6, 0xD2, 0x12, 0x13, 0xD3,
0x11, 0xD1, 0xD0, 0x10, 0xF0, 0x30, 0x31, 0xF1, 0x33, 0xF3,
0xF2, 0x32, 0x36, 0xF6, 0xF7, 0x37, 0xF5, 0x35, 0x34, 0xF4,
0x3C, 0xFC, 0xFD, 0x3D, 0xFF, 0x3F, 0x3E, 0xFE, 0xFA, 0x3A,
0x3B, 0xFB, 0x39, 0xF9, 0xF8, 0x38, 0x28, 0xE8, 0xE9, 0x29,
0xEB, 0x2B, 0x2A, 0xEA, 0xEE, 0x2E, 0x2F, 0xEF, 0x2D, 0xED,
0xEC, 0x2C, 0xE4, 0x24, 0x25, 0xE5, 0x27, 0xE7, 0xE6, 0x26,
0x22, 0xE2, 0xE3, 0x23, 0xE1, 0x21, 0x20, 0xE0, 0xA0, 0x60,
0x61, 0xA1, 0x63, 0xA3, 0xA2, 0x62, 0x66, 0xA6, 0xA7, 0x67,
0xA5, 0x65, 0x64, 0xA4, 0x6C, 0xAC, 0xAD, 0x6D, 0xAF, 0x6F,
0x6E, 0xAE, 0xAA, 0x6A, 0x6B, 0xAB, 0x69, 0xA9, 0xA8, 0x68,
0x78, 0xB8, 0xB9, 0x79, 0xBB, 0x7B, 0x7A, 0xBA, 0xBE, 0x7E,
0x7F, 0xBF, 0x7D, 0xBD, 0xBC, 0x7C, 0xB4, 0x74, 0x75, 0xB5,
0x77, 0xB7, 0xB6, 0x76, 0x72, 0xB2, 0xB3, 0x73, 0xB1, 0x71,
0x70, 0xB0, 0x50, 0x90, 0x91, 0x51, 0x93, 0x53, 0x52, 0x92,
0x96, 0x56, 0x57, 0x97, 0x55, 0x95, 0x94, 0x54, 0x9C, 0x5C,
0x5D, 0x9D, 0x5F, 0x9F, 0x9E, 0x5E, 0x5A, 0x9A, 0x9B, 0x5B,
0x99, 0x59, 0x58, 0x98, 0x88, 0x48, 0x49, 0x89, 0x4B, 0x8B,
0x8A, 0x4A, 0x4E, 0x8E, 0x8F, 0x4F, 0x8D, 0x4D, 0x4C, 0x8C,
0x44, 0x84, 0x85, 0x45, 0x87, 0x47, 0x46, 0x86, 0x82, 0x42,
0x43, 0x83, 0x41, 0x81, 0x80, 0x40
} ;
extern VEH_KWH_AC veh_kwh_ac;
unsigned int crc16(unsigned char *puchMsg, unsigned short usDataLen)
{
	unsigned char uchCRCHi=0xFF ;
	unsigned char uchCRCLo=0xFF ;
	unsigned int uIndex; 

	while(usDataLen--)
	{
		uIndex  =uchCRCLo^*puchMsg++ ;
		uchCRCLo=uchCRCHi^auchCRCHi[uIndex] ; 
		uchCRCHi=auchCRCLo[uIndex]; 
	} 

	//return (uchCRCLo<<8|uchCRCHi);
	return (uchCRCHi<<8|uchCRCLo);
}
extern uint8_t  g_rec_seq;
uint8_t Protocol_parse(uint32_t devid,uint8_t *pcmd,uint8_t *pdata,uint8_t size)
{
#define PARSE_START 	1
#define PARSE_LEN		2
#define PARSE_VER    	3
#define PARSE_ID		4
#define PARSE_SEQ    	5
#define PARSE_CMD		6
#define PARSE_DATA		7
#define PARSE_CHECK		8
	uint8_t ret = 0xFF;
	uint16_t i,j;
	uint8_t state = PARSE_START;
	uint16_t len = 0;
	uint16_t temp16 = 0;
	uint32_t temp32 = 0;
	uint16_t check;
	uint8_t buf[256];
	uint8_t index_start = 0;
	for(i = 0,j = 0; i < size; i++)
	{
		switch(state)
		{
			case PARSE_START:
				if((0xD2 == pdata[i]) && (0 == j))
				{
					j = 1;
					index_start = i;
				}
				else if((0xCF == pdata[i]) && (1 == j))
				{
					state = PARSE_LEN;
					j = 0;
				}
				else
				{
					j = 0;
				}
				break;
			case PARSE_LEN:
				if(0 == j)
				{
					temp16 += pdata[i];
					j       = 1;
				}
				else if(1 == j)
				{
					len   = (temp16 << 8) + pdata[i];
					j     = 0;

					if(len < 9)
					{
						i = size;
					}
					state = PARSE_VER;
				}
				break;
			case PARSE_VER:
				state = PARSE_ID;
				j     = 0;
				break;
			case PARSE_ID:
				if(0 == j)
				{
					temp32 = pdata[i];
					temp32 = temp32 << 8;
					j      = 1;
				}
				else if(1 == j)
				{
					temp32 += pdata[i];
					temp32  = temp32 << 8;
					j       = 2;
				}
				else if(2 == j)
				{
					temp32 += pdata[i];
					temp32  = temp32 << 8;
					j       = 3;
				}
				else if(3 == j)
				{
					temp32 += pdata[i];
					j       = 0;
					state   = PARSE_SEQ;
					if(temp32 != devid)
					{
						//i = size;
					}
				}
				break;
			case PARSE_SEQ:
				g_rec_seq = *(pdata + i);
				state = PARSE_CMD;
				break;
			case PARSE_CMD:
				*pcmd = pdata[i];
				j     = 0;
				if(9 == len)
				{
					state = PARSE_CHECK;
				}
				else
				{
					state = PARSE_DATA;
				}
				break;
			case PARSE_DATA:
				j++;
				if(j == (len - 9))
				{
					state 	= PARSE_CHECK;
					j     	= 0;
					temp16 	= 0;
				}
				break;
			case PARSE_CHECK:
				if(0 == j)
				{
					temp16 = pdata[i];
					j      = 1;
				}
				else if(1 == j)
				{
					temp16 = (temp16 << 8) + pdata[i];
					buf[0] = protocol_key >> 24;
					buf[1] = protocol_key >> 16;
					buf[2] = protocol_key >> 8;
					buf[3] = protocol_key;
					memcpy(&buf[4],&pdata[index_start],i - index_start + 1);
					check  = crc16(buf,i - index_start + 1 + 4 - 2);
					if(check == temp16)
					{
						ret = RT_EOK;
					}
					else
					{
					}
				}
				break;
		}
	}
	return ret;
}
void UploadTime(uint32_t devid,uint8_t seq,struct tm now)
{
	uint8_t buf[200];
	uint8_t i = 0;

	uint16_t temp16;
	uint32_t temp32;

	LOG_D("devid=%d, seq=%d, now: %d%d%d:%d%d%d", devid, seq,
		now.tm_year - 100, now.tm_mon + 1, now.tm_mday, now.tm_hour, now.tm_min, now.tm_sec);
	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = GETTIMEREPLY_CMD_A3;
	
	//PDU
	buf[i++] = now.tm_year - 100;				//时间				
	buf[i++] = now.tm_mon + 1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;
	
	buf[6] = 0;								//设置数据包长度
	buf[7] = i - 6;
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);	
	
}

/*
-----------------------------------------------------------------------------------
 心跳及补发上报
-----------------------------------------------------------------------------------
*/
void heart_his_data(uint32_t devid, uint8_t seq, struct tm now, uint8_t key, nmea_msg *pgps, VEHICLE *pvehicle, uint8_t cmd)
{
	uint8_t buf[200];
	uint8_t i = 0, veh_lock = ((1 == lock_vehicle) && (1 == g_lock_power));

	uint16_t temp16;
	uint32_t temp32;
	memset(buf,0xff,sizeof(buf));

	LOG_I("devid=%d, seq=%d, now=%04d-%02d-%02d,%02d:%02d:%02d, key=%d, csr=0x%08x", devid, seq,
		now.tm_year + 1900, now.tm_mon + 1, now.tm_mday, now.tm_hour, now.tm_min, now.tm_sec,
		key, csr);

	LOG_D("gps.svnum=%d, gpssta=%d, longitude=%d, latitude=%d, dir=%d, altitude=%d, speed=%d, posslnum=%d",
		pgps->svnum, pgps->gpssta, pgps->longitude, pgps->latitude, pgps->dir, pgps->altitude, pgps->speed, pgps->posslnum);

	LOG_D("vehicle.shift=%d, speed=%d, TotalMileage=%d, vol=%d, bcm.hvac=%d, bcm.key=%d, alarm=%d, veh_type=%d, veh_lock=%d",
		pvehicle->shift, pvehicle->speed, pvehicle->TotalMileage, pvehicle->vol, pvehicle->bcm.hvac, pvehicle->bcm.key, pvehicle->alarm, pvehicle->veh_type, veh_lock);

	LOG_D("vehicle.bms.VOL=%d, Current=%d, SOC=%d, gun_charge=%d, bat_in_kWh=%d, bat_out_kWh=%d, bat_plug_kWh=%d, bat_exchange_kWh=%d, bat_feedback_kWh=%d, bat_info=%d",
		pvehicle->bms.VOL, pvehicle->bms.Current, pvehicle->bms.SOC, pvehicle->bms.gun_charge,
		pvehicle->bms.bat_in_kWh, pvehicle->bms.bat_out_kWh, pvehicle->bms.bat_plug_kWh, pvehicle->bms.bat_exchange_kWh, pvehicle->bms.bat_feedback_kWh, pvehicle->bms.bat_info);

//	LOG_HEX("vehicle.bms.bat_info", sizeof(pvehicle->bms.bat_sn[0]), pvehicle->bms.bat_sn, sizeof(pvehicle->bms.bat_sn));
	LOG_D("vehicle.bms.bat_sn=%c%c%c%c%c%c%c%c%c%c%c%c", pvehicle->bms.bat_sn[0], pvehicle->bms.bat_sn[1], pvehicle->bms.bat_sn[2], pvehicle->bms.bat_sn[3], pvehicle->bms.bat_sn[4], pvehicle->bms.bat_sn[5],
		pvehicle->bms.bat_sn[6], pvehicle->bms.bat_sn[7], pvehicle->bms.bat_sn[8], pvehicle->bms.bat_sn[9], pvehicle->bms.bat_sn[10], pvehicle->bms.bat_sn[11]);

	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	if (cmd == HEART_BEAT_CMD_18) {
		buf[i++] = HEART_BEAT_CMD_18;
	} else if (cmd == HIS_HEART_CMD_29) {
		buf[i++] = HIS_HEART_CMD_29;
	}
	
	//PDU
	/* segment 1 */
	buf[i++] = now.tm_year - 100;   // 时间
	buf[i++] = now.tm_mon + 1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;
	/* segment 2 */
	buf[i++] = pvehicle->bms.SOH; //SOH
	/* segment 3 */
	buf[i++] = 0xFF;
	/* segment 4 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
	buf[i++] = 0xff;
	buf[i++] = 0xff;
	/* segment 5 */
	buf[i++] = key;//pvehicle->bcm.key; //钥匙状态
	/* segment 6 */
	buf[i++] = pvehicle->shift; //档位
	/* segment 7 */
	buf[i++] = 0xff;
	/* segment 8 */
	i += 12;
	/* segment 9 */
	i += 4;
	/* segment 10 */
	buf[i++] = 0xff;
	/* segment 11 */
	buf[i++] = pvehicle->speed; //CAN车速
	/* segment 12 */
	buf[i++] = pvehicle->TotalMileage >> 16;//总里程
	buf[i++] = pvehicle->TotalMileage >> 8;
	buf[i++] = pvehicle->TotalMileage;
	/* segment 13 */
	buf[i++] = pvehicle->vol >> 8;			//小电瓶电压
	buf[i++] = pvehicle->vol;
    /* segment 14 */
	buf[i++] = pvehicle->bms.VOL >> 8;		//总电压
	buf[i++] = pvehicle->bms.VOL;
    /* segment 15 */
	buf[i++] = pvehicle->bms.Current >> 8;	//总电流
	buf[i++] = pvehicle->bms.Current;
    /* segment 16 */
	buf[i++] = pvehicle->bms.SOC;			//剩余电量
	/* segment 17 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
    /* segment 18 */
	buf[i++] = pgps->svnum; //卫星CN 0~55
	/* segment 19 */
	buf[i++] = 0; //GPS天线状态
	/* segment 20 */
	if('A' == pgps->gpssta)
	{
		buf[i++] = 0; //卫星定位状态  0有效  1无效  FF不支持
	}
	else
	{
		buf[i++] = 1;
	}
    /* segment 21 */
	temp32   = pgps->longitude * 10;
	buf[i++] = temp32 >> 24; //经度
	buf[i++] = temp32 >> 16;
	buf[i++] = temp32 >> 8;
	buf[i++] = temp32;
    /* segment 22 */
	temp32   = pgps->latitude * 10;
	buf[i++] = temp32 >> 24; //纬度
	buf[i++] = temp32 >> 16;
	buf[i++] = temp32 >> 8;
	buf[i++] = temp32;
	/* segment 23 */
	buf[i++] =  pvehicle->bcm.hvac;
	/* segment 24 */
	buf[i++] = pvehicle->alarm;
	/* segment 25 */
	if (0 == pvehicle->bms.gun_charge) {
		buf[i++] = 0;
	}
	else if (1 == pvehicle->bms.gun_charge) {
		buf[i++] = 0x02;
	} else {
		buf[i++] = 0xff;
	}
	/* segment 26 */
	buf[i++] = 0xff;
	/* segment 27 */
	buf[i++] = 0xff;
	/* segment 28 */
	buf[i++] = 0xff;
	/* segment 29 */
	buf[i++] = pvehicle->handBrakeStatus; //手刹信号
	/* segment 30 */
	buf[i++] = 0xff;
	/* segment 31 */
	buf[i++] = 0xff;
	/* segment 32 */
	buf[i++] = 0xff;
	/* segment 33 */
	buf[i++] = veh_lock;
	/* segment 34 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
    /* segment 35 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
    /* segment 36 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
    /* segment 37 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
    /* segment 38 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
    /* segment 39 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
	/* segment 40 */
	buf[i++] = pgps->dir >> 8;				//水平航向角
	buf[i++] = pgps->dir;
    /* segment 41 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
	/* segment 42 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
    /* segment 43 */
	buf[i++] = pvehicle->veh_type;  //车辆类型
	/* segment 44 */
	buf[i++] = 0xff;
	/* segment 45 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
	/* segment 46 */
	buf[i++] = csr>>24; //重启标志位
	/* segment 47 */
	buf[i++] = pgps->altitude >> 8;			//海拔
	buf[i++] = pgps->altitude;
	/* segment 48 */
	buf[i++] = 0xff;
	buf[i++] = 0xff;
	buf[i++] = 0xff;
	buf[i++] = 0xff;
	/* segment 49 */
	buf[i++] = pvehicle->bms.bat_in_kWh >> 16;	//累计充电量
	buf[i++] = pvehicle->bms.bat_in_kWh >> 8;
	buf[i++] = pvehicle->bms.bat_in_kWh;
	/* segment 50 */
	buf[i++] = pvehicle->bms.bat_out_kWh >> 16;	//累计放电量
	buf[i++] = pvehicle->bms.bat_out_kWh >> 8;
	buf[i++] = pvehicle->bms.bat_out_kWh;
    /* segment 51 */
	buf[i++] = pvehicle->bms.bat_plug_kWh >> 16;	//累计放电量
	buf[i++] = pvehicle->bms.bat_plug_kWh >> 8;
	buf[i++] = pvehicle->bms.bat_plug_kWh;
    /* segment 52 */
	buf[i++] = pvehicle->bms.bat_exchange_kWh >> 16;	//累计放电量
	buf[i++] = pvehicle->bms.bat_exchange_kWh >> 8;
	buf[i++] = pvehicle->bms.bat_exchange_kWh;
	/* segment 53 */
	buf[i++] = pvehicle->bms.bat_feedback_kWh >> 16;	//累计放电量
	buf[i++] = pvehicle->bms.bat_feedback_kWh >> 8;
	buf[i++] = pvehicle->bms.bat_feedback_kWh;
	/* segment 54 */
	buf[i++] = (pvehicle->bms.bat_info) & 0xF8;
	buf[i++] = pvehicle->bms.bat_info & 0x07;
    
	if(((pvehicle->bms.bat_info >> 3) & 0x1F) > 27)
	{
		pvehicle->bms.bat_info = 27 + (pvehicle->bms.bat_info & 0x07);
	}
	memcpy(&buf[i],pvehicle->bms.bat_sn,(pvehicle->bms.bat_info >> 3) & 0x1F);
	i = i + ((pvehicle->bms.bat_info >> 3) & 0x1F);
	buf[6] = 0; //设置数据包长度
	buf[7] = i - 6;
    
	temp16 = crc16(buf,i); //校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	if (cmd == HEART_BEAT_CMD_18) {
		MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG, (uint8_t *)&buf[4], i - 4);
	} else if (cmd == HIS_HEART_CMD_29) {
		if (his_data.tail >= (0x400 + 200 * HIS_DATA_MAX)) {
			his_data.tail = 0x400;
		}
		FM24CLXX_Write(his_data.tail, &buf[4], 200);
		his_data.cnt++;
		if(his_data.cnt >= HIS_DATA_MAX)
		{
			his_data.cnt = HIS_DATA_MAX;
		}
		his_data.tail += 200;

		PrintMobileSendData(&buf[4], 200);
		FM24CLXX_Write(HISDATA_START_ADDR, (uint8_t *)&his_data, sizeof(LOG_MANAGER));
	}
}
/*
-----------------------------------------------------------------------------------
 将ASCII字符串转为BCD
-----------------------------------------------------------------------------------
*/
uint8_t atoBCD(uint8_t *pdest, const char *psrc,int size)
{
	uint8_t ret = 0;
	uint8_t len;
	uint8_t i;
	uint8_t hi;
	uint8_t lo;
	len = strlen(psrc);
	

		for(i = 0; i < size; i++)
		{
			hi = psrc[i * 2];
			lo = psrc[i * 2 + 1] ;
			if((hi >= '0') && (hi <= '9'))
			{
				hi -= '0';
			}
			else if((hi >= 'a') && (hi <= 'f'))
			{
				hi = hi - 'a' + 10;
			}
			else if((hi >= 'A') && (hi <= 'F'))
			{
				hi = hi - 'A' + 10;
			}
			
			if((lo >= '0') && (lo <= '9'))
			{
				lo -= '0';
			}
			else if((lo >= 'a') && (lo <= 'f'))
			{
				lo = lo - 'a' + 10;
			}
			else if((lo >= 'A') && (lo <= 'F'))
			{
				lo = lo - 'A' + 10;
			}		
			pdest[i] = (hi << 4) + lo;
		}
		ret = 1;

	return ret;
}
/*
	设备登出,cmd:0x2b
*/
void devLogOut(uint32_t devid,uint8_t seq,uint8_t result){
	uint8_t buf[300];
	uint8_t i = 0;
	struct tm now;
	time_t t_now;
	t_now = time(NULL) + (RT_LIBC_DEFAULT_TIMEZONE * ONEHOUR_SEC);
	uint16_t temp16;
	uint32_t temp32;
	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = DEVLOGOUT_CMD_2B;
	now = *localtime(&t_now);
	buf[i++] = now.tm_year - 100;
	buf[i++] = now.tm_mon+1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;

	buf[i++] = result;	//登出原因


	buf[6] = 0;								//设置数据包长度
	buf[7] = i - 6;
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);	
}
/*
-----------------------------------------------------------------------------------
 设备信息上报，初始登陆cmd使用0X1B,服务器请求设备信息cmd使用0xA1
-----------------------------------------------------------------------------------
*/
void UploadDevInfo(uint32_t devid,uint8_t seq,uint8_t cmd,uint16_t hardver,uint16_t softver,const char *psim,const char *pvin,VEHICLE *pvehicle)
{
	uint8_t buf[200];
	uint8_t i = 0;
	uint8_t ret;
	uint16_t temp16;
	uint32_t temp32;

	LOG_D("devid=%08d, seq=%d, cmd=0x%02x, hardver=0x%04x, softver=0x%04x", devid, seq, cmd, hardver, softver);
	LOG_D("psim=%s, pvin=%s", psim, pvin);

	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = cmd;
	
	buf[i++] = hardver >> 8;
	buf[i++] = hardver;
	
	buf[i++] = 0xFF;//(softver >> 16) & 0xff
	buf[i++] = softver >> 8;
	buf[i++] = softver;
	
	ret = atoBCD(&buf[i],psim,10);
	if(ret)
	{
		i += 10;
	}
	else
	{
		rt_kprintf("sim err\r\n");
		return;
	}
	memset(&buf[i],0xff,6);
	i += 6;
	memcpy(&buf[i],pvin,17);
	i += 17;
	
	buf[i++] = (pvehicle->VCUVersionHigh)>>24;
	buf[i++] = (pvehicle->VCUVersionHigh)>>16;
	buf[i++] = (pvehicle->VCUVersionHigh)>>8;
	buf[i++] = pvehicle->VCUVersionHigh;
  buf[i++] = (pvehicle->VCUVersionLow)>>24;
	buf[i++] = (pvehicle->VCUVersionLow)>>16;
	buf[i++] = (pvehicle->VCUVersionLow)>>8;
	buf[i++] = pvehicle->VCUVersionLow;
	
	buf[6] = 0;								//设置数据包长度
	buf[7] = i - 6;
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);	
}
void UploadRply(uint32_t devid,uint8_t seq,uint8_t cmd,uint8_t rply)
{
	uint8_t buf[100];
	uint8_t i = 0;
	uint8_t ret;
	uint16_t temp16;
	uint32_t temp32;
	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = cmd;
	
	buf[i++] = rply;
	
	buf[6] = 0;								//设置数据包长度
	buf[7] = i - 6;
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);	
}
void UploadFault(uint32_t devid,uint8_t seq,time_t faulttime,uint8_t *pdata,uint8_t data)
{
	uint8_t buf[200];
	uint8_t i = 0;
	uint8_t ret;
	uint16_t temp16;
	uint32_t temp32;
	struct tm now;
	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = FAULT_CMD_25;
	faulttime += (RT_LIBC_DEFAULT_TIMEZONE * ONEHOUR_SEC);
	now = *localtime(&faulttime);
	buf[i++] = now.tm_year - 100;
	buf[i++] = now.tm_mon+1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;
	buf[i++] = *pdata;
	buf[i++] = *(pdata + 1);
	buf[i++] = *(pdata + 2);
	buf[i++] = *(pdata + 3);
	buf[i++] = *(pdata + 4);
	buf[i++] = *(pdata + 5);
	buf[i++] = *(pdata + 6);
	buf[i++] = *(pdata + 7);
    buf[i++] = data;    
	
	buf[6] = 0;								//设置数据包长度
	buf[7] = i - 6;
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);	
}
void UploadFileRply(uint32_t devid,uint8_t seq,uint8_t cmd,uint8_t rply,uint16_t totalfile)
{
	uint8_t buf[100];
	uint8_t i = 0;
	uint8_t ret;
	uint16_t temp16;
	uint32_t temp32;
	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = cmd;
	
	buf[i++] = rply;
	buf[i++] = totalfile >> 8;
	buf[i++] = totalfile;	
	
	buf[6] = 0;								//设置数据包长度
	buf[7] = i - 6;
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);	
}
void UploadFileRate(uint32_t devid,uint8_t seq,time_t time_now,uint16_t curfile,uint16_t totalfile,uint8_t result)
{
	uint8_t buf[200];
	uint8_t i = 0;
	uint8_t ret;
	uint16_t temp16;
	uint32_t temp32;
	struct tm now;
	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = UP_FILE_RATE_CMD_28;
	time_now += (RT_LIBC_DEFAULT_TIMEZONE * ONEHOUR_SEC);
	now = *localtime(&time_now);
	buf[i++] = now.tm_year - 100;
	buf[i++] = now.tm_mon+1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;
	buf[i++] = curfile >> 8;
	buf[i++] = curfile;
	buf[i++] = totalfile >> 8;
	buf[i++] = totalfile;
	buf[i++] = result;
	
	buf[6] = 0;								//设置数据包长度
	buf[7] = i - 6;
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);		
}
void UploadExchange(uint32_t devid,uint8_t seq,time_t time_now,uint8_t wifi_state,uint8_t wifi_sn[],uint8_t ver,uint8_t wifi_cmd,uint8_t con1,uint8_t con2,uint8_t lock_state,uint8_t unlock_state,uint8_t m_switch,int8_t temp[],uint8_t alarm,uint8_t bat_info,int8_t bat_sn[])
{
	uint8_t buf[200];
	uint8_t i = 0;
	uint8_t ret;
	uint16_t temp16;
	uint32_t temp32;
	struct tm now;
	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = WIFI_CONNECT_CMD_26;
	time_now += (RT_LIBC_DEFAULT_TIMEZONE * ONEHOUR_SEC);
	now = *localtime(&time_now);
	buf[i++] = now.tm_year - 100;
	buf[i++] = now.tm_mon+1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;

	buf[i++] = !wifi_state;					//换电站WIFI连接状态
	memcpy(&buf[i],wifi_sn,15);				//换电站编号
	i += 15;
	
	buf[i++] = ver;                     	//换电站协议版本
	buf[i++] = wifi_cmd;					//换电站WIFI控制电磁锁
	buf[i++] = con1;
	buf[i++] = con2;
	buf[i++] = lock_state;
	buf[i++] = unlock_state;
	buf[i++] = m_switch;
	memcpy(&buf[i],temp,8);
	i += 8;
	
	buf[i++] = alarm;
	buf[i++] = bat_info;
	memcpy(&buf[i],bat_sn,(bat_info >> 3) & 0x1F);
	i = i + ((bat_info >> 3) & 0x1F);
	buf[i++] = veh_kwh_ac.veh_out_kwh >> 16;
	buf[i++] = veh_kwh_ac.veh_out_kwh >> 8;
	buf[i++] = veh_kwh_ac.veh_out_kwh;
	buf[i++] = veh_kwh_ac.veh_fb_kwh >> 16;
	buf[i++] = veh_kwh_ac.veh_fb_kwh >> 8;
	buf[i++] = veh_kwh_ac.veh_fb_kwh;
	buf[i++] = veh_kwh_ac.veh_total_ei >> 16;
	buf[i++] = veh_kwh_ac.veh_total_ei >> 8;
	buf[i++] = veh_kwh_ac.veh_total_ei;
	buf[i++] = veh_kwh_ac.veh_plug_kwh >> 16;
	buf[i++] = veh_kwh_ac.veh_plug_kwh >> 8;
	buf[i++] = veh_kwh_ac.veh_plug_kwh;
	buf[i++] = veh_kwh_ac.rt_out_kwh >> 16;
	buf[i++] = veh_kwh_ac.rt_out_kwh >> 8;
	buf[i++] = veh_kwh_ac.rt_out_kwh;
	buf[i++] = veh_kwh_ac.last_out_kwh >> 16;
	buf[i++] = veh_kwh_ac.last_out_kwh >> 8;
	buf[i++] = veh_kwh_ac.last_out_kwh;
	buf[i++] = veh_kwh_ac.rt_fb_kwh >> 16;
	buf[i++] = veh_kwh_ac.rt_fb_kwh >> 8;
	buf[i++] = veh_kwh_ac.rt_fb_kwh;
	buf[i++] = veh_kwh_ac.last_fb_kwh >> 16;
	buf[i++] = veh_kwh_ac.last_fb_kwh >> 8;
	buf[i++] = veh_kwh_ac.last_fb_kwh;
	buf[i++] = veh_kwh_ac.rt_plug_kwh >> 16;
	buf[i++] = veh_kwh_ac.rt_plug_kwh >> 8;
	buf[i++] = veh_kwh_ac.rt_plug_kwh;
	buf[i++] = veh_kwh_ac.last_plug_kwh >> 16;
	buf[i++] = veh_kwh_ac.last_plug_kwh >> 8;
	buf[i++] = veh_kwh_ac.last_plug_kwh;
	buf[i++] = veh_kwh_ac.rt_total_ei >> 16;
	buf[i++] = veh_kwh_ac.rt_total_ei >> 8;
	buf[i++] = veh_kwh_ac.rt_total_ei;
	buf[i++] = veh_kwh_ac.last_total_ei >> 16;
	buf[i++] = veh_kwh_ac.last_total_ei >> 8;
	buf[i++] = veh_kwh_ac.last_total_ei;
	buf[i++] = veh_kwh_ac.rt_count_elec >> 16;
	buf[i++] = veh_kwh_ac.rt_count_elec >> 8;
	buf[i++] = veh_kwh_ac.rt_count_elec;
	time_now = veh_kwh_ac.exchange;
	time_now += (RT_LIBC_DEFAULT_TIMEZONE * ONEHOUR_SEC);
	now = *localtime(&time_now);
	buf[i++] = now.tm_year - 100;
	buf[i++] = now.tm_mon+1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;
	buf[6] = 0;								//设置数据包长度
	buf[7] = i - 6;
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);		
}

#ifdef ECU_UPDATE_ENABLE
void UploadECUUpdateProgress(uint32_t devid,uint8_t seq,time_t time_now,uint8_t cur_state,uint32_t para_value_high,uint32_t para_value_low)
{
	uint8_t buf[200];
	uint8_t i = 0;
	uint8_t ret;
	uint16_t temp16;
	uint32_t temp32;
	struct tm now;
	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = ECU_UPDATE_PROGRASS_CMD_2A;
	time_now += (RT_LIBC_DEFAULT_TIMEZONE * ONEHOUR_SEC);
	now = *localtime(&time_now);
	buf[i++] = now.tm_year - 100;
	buf[i++] = now.tm_mon+1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;

	buf[i++] = cur_state;       //当前状态
	buf[i++] = para_value_high >> 24; //参数内容高位
	buf[i++] = para_value_high >> 16;
	buf[i++] = para_value_high >> 8;
	buf[i++] = para_value_high;
	buf[i++] = para_value_low >> 24; //参数内容低位
	buf[i++] = para_value_low >> 16;
	buf[i++] = para_value_low >> 8;
	buf[i++] = para_value_low;
	
	buf[6] = 0;								//设置数据包长度
	buf[7] = i - 6;
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);	
}
#endif
extern struct electric_linear_actuator_position ele_machine_pos;
extern uint16_t dis_fault_flag;
extern uint8_t g_no_sleep_flag; //0 休眠  1不休眠  默认不休眠
extern uint8_t g_lock_power;    //锁车标志，默认不锁车  0不锁车  1锁车
void UploadVehiPara(uint32_t devid,uint8_t seq,VEHICLE *pvehicle)
{
	uint8_t buf[200];
	uint8_t i = 0;
	uint8_t ret;
	uint16_t len_ela = 0; /* electric linear actuator byte */
	uint16_t temp16;
	uint32_t temp32;
	struct tm now;
	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = CHECKVEHICLEPARAREPLY_CMD_B2;

	buf[i++] = 0xff;       
	buf[i++] = 0xff; 
	buf[i++] = 0xff;
	buf[i++] = 0xff;       
	buf[i++] = 0xff; 
	buf[i++] = 0xff;	
	buf[i++] = 0xff;       
	buf[i++] = 0xff; 
	buf[i++] = 0xff;
	buf[i++] = 0xff;       
	buf[i++] = 0xff; 
	buf[i++] = 0xff;
	buf[i++] = 0xff;       
	buf[i++] = 0xff; 
	buf[i++] = 0xff;
	buf[i++] = 0xff;       
	buf[i++] = 0xff; 
	buf[i++] = 0xff;
	buf[i++] = 0xff;       
	buf[i++] = 0xff; 
	buf[i++] = 0xff;
	buf[i++] = 0xff;       
	buf[i++] = 0xff; 
	buf[i++] = 0xff;
	buf[i++] = 0xff;       
	buf[i++] = 0xff; 
	buf[i++] = 0xff;
	buf[i++] = 0xff;       
	buf[i++] = 0xff; 
	buf[i++] = pvehicle->temp_th_lv.th_lv_buf[0];
	buf[i++] = pvehicle->temp_th_lv.th_lv_buf[1];
	buf[i++] = pvehicle->temp_th_lv.th_lv_buf[2];
	buf[i++] = pvehicle->temp_th_lv.th_lv_buf[3];
	buf[i++] = pvehicle->temp_th_lv.th_lv_buf[3];
	buf[i++] = pvehicle->temp_th_lv.th_lv_buf[4];
	buf[i++] = pvehicle->temp_th_lv.th_lv_buf[5];
	buf[i++] = dis_fault_flag >> 8;
	buf[i++] = dis_fault_flag;
	buf[i++] = (pvehicle->VCUVersionHigh)>>24;
	buf[i++] = (pvehicle->VCUVersionHigh)>>16;
	buf[i++] = (pvehicle->VCUVersionHigh)>>8;
	buf[i++] = pvehicle->VCUVersionHigh;
    buf[i++] = (pvehicle->VCUVersionLow)>>24;
	buf[i++] = (pvehicle->VCUVersionLow)>>16;
	buf[i++] = (pvehicle->VCUVersionLow)>>8;
	buf[i++] = pvehicle->VCUVersionLow;	
	buf[i++] = g_no_sleep_flag;
#ifdef FUTIAN_CAN_TRANSFER    
    extern uint8_t g_transmit_flag;
    buf[i++] = g_transmit_flag;
#else
    buf[i++] = 1;
#endif
    buf[i++] = g_lock_power;
    buf[i++] = (uint8_t)getMagnetMode();
#ifdef ELECTRIC_LINEAR_ACTUATOR_USE
    buf[i++] = ele_machine_pos.act_lock_min_pos;
    buf[i++] = ele_machine_pos.act_lock_max_pos;
    buf[i++] = ele_machine_pos.act_unlock_min_pos;
    buf[i++] = ele_machine_pos.act_unlock_max_pos;
    buf[i++] = ele_machine_pos.act_unlock_tar_pos;
    buf[i++] = ele_machine_pos.act_lock_tar_pos;
#else
    buf[i++] = 0xff;
    buf[i++] = 0xff;
    buf[i++] = 0xff;
    buf[i++] = 0xff;
    buf[i++] = 0xff;
    buf[i++] = 0xff;
#endif
    buf[6] = 0;								//设置数据包长度
    buf[7] = i - 6;
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);	  
}

#if VEHICLE_TYPE == _MINING_TRUCK
void UploadVehWeight(uint32_t devid, uint8_t seq, struct tm now, VEHICLE *pvehicle)
{
	uint8_t buf[31];
	uint8_t i = 0;
	uint16_t temp16;
	memset(buf,0xff, sizeof(buf));

	LOG_I("veh_weight is %d T", pvehicle->veh_weight);

	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;

	temp16 = PROTOCOL_SYN;
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	i++;
	i++;

	buf[i++] = PROTOCOL_VER;

	buf[i++] = devid >> 24;
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;

	buf[i++] = seq;

	buf[i++] = UPLOAD_VEH_WEIGHT;

	buf[i++] = now.tm_year - 100;
	buf[i++] = now.tm_mon+1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;

	buf[i++] = pvehicle->veh_weight;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;

	buf[6] = 0;
	buf[7] = i - 6;

	temp16 = crc16(buf, i);
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD, CLIENT_MD, DATA_MSG, (uint8_t *)&buf[4], i - 4);	
}
#endif

bool SendHisData(void)
{
    uint8_t buf[200];
    uint16_t protocol_head;
    
    if(his_data.cnt > 0)
    {
        FM24CLXX_Read(his_data.head,buf,200);
        protocol_head = (buf[0] << 8) + buf[1];
        
        if (protocol_head == PROTOCOL_SYN)
        {
            MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,buf,buf[3]+4);
            return true;
        }
    }
    
    return false;
}

void UploadBMSData(uint32_t devid, uint8_t seq, struct tm now, VEHICLE *pvehicle)
{
	uint8_t buf[82];
	uint8_t i = 0;
	uint16_t temp16;
	memset(buf, 0xff, sizeof(buf));

	LOG_I("bms.master_neg_relay_status is %d", 
		pvehicle->bms.master_neg_relay_status);    
	LOG_I("bms.software version is %d%d.%d%d.%d%d", 
		pvehicle->bms.sftw_vrs[0], pvehicle->bms.sftw_vrs[1], pvehicle->bms.sftw_vrs[2], 
		pvehicle->bms.sftw_vrs[3], pvehicle->bms.sftw_vrs[4], pvehicle->bms.sftw_vrs[5]);        
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;

	temp16 = PROTOCOL_SYN;
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	i++;
	i++;

	buf[i++] = PROTOCOL_VER;

	buf[i++] = devid >> 24;
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;

	buf[i++] = seq;

	buf[i++] = UPLOAD_BMS_DATA;

	buf[i++] = now.tm_year - 100;
	buf[i++] = now.tm_mon+1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;

	buf[i++] = pvehicle->bms.cell_temp_max;
	buf[i++] = pvehicle->bms.cell_temp_min;
	buf[i++] = pvehicle->bms.cell_temp_ave;
	buf[i++] = pvehicle->bms.cell_temp_max_csc;
	buf[i++] = pvehicle->bms.cell_temp_max_csc_index;
	buf[i++] = pvehicle->bms.cell_temp_min_csc;
	buf[i++] = pvehicle->bms.cell_temp_min_csc_index;
	buf[i++] = pvehicle->bms.cell_vol_max_csc;
	buf[i++] = pvehicle->bms.cell_vol_max_csc_index;
	buf[i++] = pvehicle->bms.cell_vol_min_csc;
	buf[i++] = pvehicle->bms.cell_vol_min_csc_index;
	buf[i++] = 0xFF;
	buf[i++] = pvehicle->bms.bat_pos_insulation >> 8;
	buf[i++] = pvehicle->bms.bat_pos_insulation;
	buf[i++] = pvehicle->bms.bat_neg_insulation >> 8;
	buf[i++] = pvehicle->bms.bat_neg_insulation;
	buf[i++] = pvehicle->bms.bat_vol >> 8;
	buf[i++] = pvehicle->bms.bat_vol;
	buf[i++] = pvehicle->bms.bus_vol >> 8;
	buf[i++] = pvehicle->bms.bus_vol;
	buf[i++] = pvehicle->bms.cell_vol_max >> 8;
	buf[i++] = pvehicle->bms.cell_vol_max;
	buf[i++] = pvehicle->bms.cell_vol_ave >> 8;
	buf[i++] = pvehicle->bms.cell_vol_ave;
	buf[i++] = pvehicle->bms.cell_vol_min >> 8;
	buf[i++] = pvehicle->bms.cell_vol_min;

	buf[i++] = pvehicle->bms.sftw_vrs[0];//bms version
	buf[i++] = pvehicle->bms.sftw_vrs[1];
	buf[i++] = pvehicle->bms.sftw_vrs[2];
	buf[i++] = pvehicle->bms.sftw_vrs[3];    
	buf[i++] = pvehicle->bms.sftw_vrs[4];
	buf[i++] = pvehicle->bms.sftw_vrs[5];
	
	buf[i++] = (pvehicle->bms.master_neg_relay_status << 6) & 0xC0;//主负继电器状态
	
	buf[i++] = 0xFF;//BBOX ID (reserved)
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	
	buf[i++] = 0xFF;//reserved
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;    

	buf[6] = 0;
	buf[7] = i - 6;

	temp16 = crc16(buf, i);
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD, CLIENT_MD, DATA_MSG, (uint8_t *)&buf[4], i - 4);	
}

#if VEHICLE_TYPE == _MECHANICAL_VER
void UploadWorkingTime(uint32_t devid, uint8_t seq, struct tm now, VEHICLE *pvehicle)
{
	uint8_t buf[47];
	uint8_t i = 0;
	uint16_t temp16;
	memset(buf,0xff, sizeof(buf));

	LOG_I("machine_high_voltage_period is %d, machine_high_voltage_status is %d, machine_motor_status is %d", 
		pvehicle->mc_hv_period, pvehicle->mc_hv_status, pvehicle->mc_mt_status);
	LOG_I("machine_high_voltage_time is %02d%d.%02d.%02d.%02d:%02d:%02d, machine_low_voltage_time is %02d%d.%02d.%02d.%02d:%02d:%02d", 
		pvehicle->mc_hv_time[0], pvehicle->mc_hv_time[1], pvehicle->mc_hv_time[2], pvehicle->mc_hv_time[3],
		pvehicle->mc_hv_time[4], pvehicle->mc_hv_time[5], pvehicle->mc_hv_time[6], 
		pvehicle->mc_lv_time[0], pvehicle->mc_lv_time[1], pvehicle->mc_lv_time[2], pvehicle->mc_lv_time[3],
		pvehicle->mc_lv_time[4], pvehicle->mc_lv_time[5], pvehicle->mc_lv_time[6]);    

	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;

	temp16 = PROTOCOL_SYN;
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	i++;
	i++;

	buf[i++] = PROTOCOL_VER;

	buf[i++] = devid >> 24;
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;

	buf[i++] = seq;

	buf[i++] = UPLOAD_VEH_WORK_DATA;

	buf[i++] = now.tm_year - 100;
	buf[i++] = now.tm_mon+1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;

	buf[i++] = pvehicle->mc_hv_period >> 24;
	buf[i++] = pvehicle->mc_hv_period >> 16;
	buf[i++] = pvehicle->mc_hv_period >> 8;
	buf[i++] = pvehicle->mc_hv_period;
	
	buf[i++] = pvehicle->mc_hv_status;
	
	buf[i++] = pvehicle->mc_hv_time[0];
	buf[i++] = pvehicle->mc_hv_time[1];
	buf[i++] = pvehicle->mc_hv_time[2];
	buf[i++] = pvehicle->mc_hv_time[3];
	buf[i++] = pvehicle->mc_hv_time[4];
	buf[i++] = pvehicle->mc_hv_time[5];
	buf[i++] = pvehicle->mc_hv_time[6];
	buf[i++] = pvehicle->mc_hv_time[7]; 
	
	buf[i++] = pvehicle->mc_lv_time[0];
	buf[i++] = pvehicle->mc_lv_time[1];
	buf[i++] = pvehicle->mc_lv_time[2];
	buf[i++] = pvehicle->mc_lv_time[3];
	buf[i++] = pvehicle->mc_lv_time[4];
	buf[i++] = pvehicle->mc_lv_time[5];
	buf[i++] = pvehicle->mc_lv_time[6];
	buf[i++] = pvehicle->mc_lv_time[7]; 
	
	buf[i++] = pvehicle->mc_mt_status;
	
	buf[i++] = 0xFF;
	buf[i++] = 0xFF;
	
	buf[6] = 0;
	buf[7] = i - 6;

	temp16 = crc16(buf, i);
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD, CLIENT_MD, DATA_MSG, (uint8_t *)&buf[4], i - 4);	
}
#endif

/* Cut 13: sequence number; Cut 18 -- 22: hour,minute,second; 26--29: heart count  total len: 37 */
/*   be checked buf 0--12,14--17, 25--28, 29--37 */
static int cmp_upload_send_buf(const unsigned char *pbuf, const uint8_t len)
{
	static unsigned char re_buf[100];

	if (!memcmp(re_buf, pbuf, 13) && 
		!memcmp(re_buf + 14, pbuf + 14, 4) && 
		!memcmp(re_buf + 22, pbuf + 22, 4) && 
		!memcmp(re_buf + 29, pbuf + 29, len - 29)) {
		return 0;
	} else {
		memcpy(re_buf, pbuf, len);
		return 1;
	}
}
/*
-----------------------------------------------------------------------------------
 终端连接换电站信息
-----------------------------------------------------------------------------------
*/
void WifiMessageSend(uint32_t devid,uint8_t seq,struct tm now,uint8_t key,WIFI_MSG_T *pwifi)
{
	uint8_t buf[100];
	uint8_t i = 0;

	uint16_t temp16;
	uint32_t temp32;

	memset(buf,0xff,sizeof(buf));
	
	buf[i++] = protocol_key >> 24;
	buf[i++] = protocol_key >> 16;
	buf[i++] = protocol_key >> 8;
	buf[i++] = protocol_key;
	
	temp16 = PROTOCOL_SYN;					//帧起始标志
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;
	
	i++;	//帧长度，先占空间，从协议版本到校验的字节数
	i++;
	
	buf[i++] = PROTOCOL_VER;				//协议版本
	
	buf[i++] = devid >> 24;					//设备ID
	buf[i++] = devid >> 16;
	buf[i++] = devid >> 8;
	buf[i++] = devid;
	
	buf[i++] = seq;
	
	buf[i++] = WIFI_MESSAGE_CMD_2C;
	
	//PDU
	buf[i++] = now.tm_year - 100;				//时间				
	buf[i++] = now.tm_mon + 1;
	buf[i++] = now.tm_mday;
	buf[i++] = now.tm_hour;
	buf[i++] = now.tm_min;
	buf[i++] = now.tm_sec;
	
	buf[i++] = key;
    
    buf[i++] = pwifi->wifi_module_state;
    buf[i++] = pwifi->get_ip_state;
    buf[i++] = pwifi->tcp_con_state;
    buf[i++] = pwifi->send_heart_cnt >> 8;
    buf[i++] = pwifi->send_heart_cnt;
    buf[i++] = pwifi->rec_heart_ack_cnt >> 8;
    buf[i++] = pwifi->rec_heart_ack_cnt;
    buf[i++] = (pwifi->rec_unlock_cnt & 0x0f) + ((pwifi->send_unlock_ack_cnt & 0x0f) << 4);
    buf[i++] = (pwifi->rec_lock_cnt & 0x0f) + ((pwifi->send_lock_ack_cnt & 0x0f) << 4);
    buf[i++] = pwifi->unlock_state;
    buf[i++] = pwifi->fa_state;
    buf[i++] = pwifi->con1_state;
    buf[i++] = pwifi->con2_state;  
    buf[i++] = pwifi->fa_recheck;
    buf[i++] = pwifi->unlock_recheck;

	buf[6] = 0;								//设置数据包长度
	buf[7] = i - 6;
    
	upload_send_change_flag = cmp_upload_send_buf((const unsigned char *)buf, i);

	LOG_CHG(upload_send_change_flag, "devid=%d, seq=%d, now=%d-%02d-%02d %02d:%02d:%02d", devid, seq,
		now.tm_year - 100, now.tm_mon + 1, now.tm_mday, now.tm_hour, now.tm_min, now.tm_sec);
	LOG_CHG(upload_send_change_flag, "wifi_module_state=%d, get_ip_state=%d, tcp_con_state=%d, send_heart_cnt=%d, rec_heart_ack_cnt=%d",
		pwifi->wifi_module_state, pwifi->get_ip_state, pwifi->tcp_con_state, pwifi->send_heart_cnt, pwifi->rec_heart_ack_cnt);
	LOG_CHG(upload_send_change_flag, "rec_unlock_cnt=%d, send_unlock_ack_cnt=%d, unlock_state=%d, fa_state=%d, con1_state=%d, con2_state=%d, fa_recheck=%d, unlock_recheck=%d",
		pwifi->rec_unlock_cnt, pwifi->send_unlock_ack_cnt, pwifi->unlock_state, pwifi->fa_state, pwifi->con1_state, pwifi->con2_state, pwifi->fa_recheck, pwifi->unlock_recheck);  
    
	temp16 = crc16(buf,i);					//校验
	buf[i++] = temp16 >> 8;
	buf[i++] = temp16;

	MxPutMsgEx(MOBILE_MD,CLIENT_MD,DATA_MSG,(uint8_t *)&buf[4],i - 4);
}
